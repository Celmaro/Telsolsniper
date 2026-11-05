use anyhow::{anyhow, bail, Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::HashSet,
    env,
    os::unix::fs::{FileTypeExt, PermissionsExt},
    path::Path,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::{UnixListener, UnixStream},
};
use tracing::{error, info, warn};

const MAX_LINE_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone)]
struct Config {
    socket_path: String,
    database_path: String,
    allowed_channels: HashSet<String>,
}

impl Config {
    fn from_env() -> Result<Self> {
        let socket_path =
            env::var("SIGNAL_SOCKET_PATH").unwrap_or_else(|_| "/tmp/telsolsniper.sock".into());
        let database_path =
            env::var("SIGNAL_DATABASE_PATH").unwrap_or_else(|_| "./data/signals.sqlite".into());
        let channels = env::var("ALLOWED_CHANNELS").unwrap_or_else(|_| {
            "soltrenchtrending,solwhaletrending,solearlytrending".into()
        });
        let allowed_channels = channels
            .split(',')
            .map(|value| value.trim().trim_start_matches('@').to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect::<HashSet<_>>();

        if allowed_channels.is_empty() {
            bail!("ALLOWED_CHANNELS must contain at least one channel username");
        }

        Ok(Self {
            socket_path,
            database_path,
            allowed_channels,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TelegramSignal {
    source: String,
    message_id: i64,
    mint: String,
    ticker: Option<String>,
    message_text: String,
    telegram_date_unix_ms: Option<i64>,
    received_at_unix_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IngestOutcome {
    NewMint,
    DuplicateMint,
    DuplicateMessage,
}

struct SignalStore {
    connection: Mutex<Connection>,
}

impl SignalStore {
    fn open(path: &str) -> Result<Self> {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create database directory {}", parent.display()))?;
            }
        }

        let connection = Connection::open(path)
            .with_context(|| format!("open signal database at {path}"))?;
        connection.busy_timeout(Duration::from_secs(2))?;
        connection.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;

            CREATE TABLE IF NOT EXISTS telegram_messages (
                source TEXT NOT NULL,
                message_id INTEGER NOT NULL,
                mint TEXT NOT NULL,
                ticker TEXT,
                message_text TEXT NOT NULL,
                telegram_date_unix_ms INTEGER,
                received_at_unix_ms INTEGER NOT NULL,
                outcome TEXT NOT NULL,
                PRIMARY KEY (source, message_id, mint)
            );

            CREATE TABLE IF NOT EXISTS mint_claims (
                mint TEXT PRIMARY KEY,
                first_source TEXT NOT NULL,
                first_message_id INTEGER NOT NULL,
                claimed_at_unix_ms INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_telegram_messages_mint
                ON telegram_messages(mint);
            "#,
        )?;

        // A trending-leaderboard post announces several mints at once, so the
        // row key must include the mint. Migrate databases created before that
        // change (their PK is source+message_id and multi-mint posts are lost).
        let mint_in_primary_key: i64 = connection.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('telegram_messages') WHERE name = 'mint' AND pk > 0",
            [],
            |row| row.get(0),
        )?;
        if mint_in_primary_key == 0 {
            connection.execute_batch(
                r#"
                ALTER TABLE telegram_messages RENAME TO telegram_messages_legacy;
                CREATE TABLE telegram_messages (
                    source TEXT NOT NULL,
                    message_id INTEGER NOT NULL,
                    mint TEXT NOT NULL,
                    ticker TEXT,
                    message_text TEXT NOT NULL,
                    telegram_date_unix_ms INTEGER,
                    received_at_unix_ms INTEGER NOT NULL,
                    outcome TEXT NOT NULL,
                    PRIMARY KEY (source, message_id, mint)
                );
                INSERT OR IGNORE INTO telegram_messages
                    SELECT source, message_id, mint, ticker, message_text,
                           telegram_date_unix_ms, received_at_unix_ms, outcome
                    FROM telegram_messages_legacy;
                DROP TABLE telegram_messages_legacy;
                CREATE INDEX IF NOT EXISTS idx_telegram_messages_mint
                    ON telegram_messages(mint);
                "#,
            )?;
        }

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn ingest(&self, signal: &TelegramSignal) -> Result<IngestOutcome> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| anyhow!("signal database mutex is poisoned"))?;
        let transaction = connection.transaction()?;

        let inserted = transaction.execute(
            r#"
            INSERT INTO telegram_messages (
                source, message_id, mint, ticker, message_text,
                telegram_date_unix_ms, received_at_unix_ms, outcome
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'received')
            ON CONFLICT(source, message_id, mint) DO NOTHING
            "#,
            params![
                signal.source,
                signal.message_id,
                signal.mint,
                signal.ticker,
                signal.message_text,
                signal.telegram_date_unix_ms,
                signal.received_at_unix_ms
            ],
        )?;

        if inserted == 0 {
            transaction.commit()?;
            return Ok(IngestOutcome::DuplicateMessage);
        }

        let claimed = transaction.execute(
            r#"
            INSERT INTO mint_claims (
                mint, first_source, first_message_id, claimed_at_unix_ms
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(mint) DO NOTHING
            "#,
            params![
                signal.mint,
                signal.source,
                signal.message_id,
                signal.received_at_unix_ms
            ],
        )?;

        let outcome = if claimed == 1 {
            IngestOutcome::NewMint
        } else {
            IngestOutcome::DuplicateMint
        };
        let outcome_text = match outcome {
            IngestOutcome::NewMint => "new_mint",
            IngestOutcome::DuplicateMint => "duplicate_mint",
            IngestOutcome::DuplicateMessage => "duplicate_message",
        };

        transaction.execute(
            "UPDATE telegram_messages SET outcome = ?1 WHERE source = ?2 AND message_id = ?3 AND mint = ?4",
            params![outcome_text, signal.source, signal.message_id, signal.mint],
        )?;
        transaction.commit()?;

        Ok(outcome)
    }
}

fn validate_signal(mut signal: TelegramSignal, config: &Config) -> Result<TelegramSignal> {
    signal.source = signal
        .source
        .trim()
        .trim_start_matches('@')
        .to_ascii_lowercase();

    if !config.allowed_channels.contains(&signal.source) {
        bail!("message source is not one of the configured Telegram channels");
    }
    if signal.message_id <= 0 {
        bail!("Telegram message_id must be positive");
    }
    if Pubkey::from_str(&signal.mint).is_err() {
        bail!("mint is not a valid Solana public key");
    }
    if signal.message_text.len() > 8 * 1024 {
        bail!("Telegram message text exceeds the 8 KiB limit");
    }
    if signal.received_at_unix_ms <= 0 {
        bail!("receivedAtUnixMs must be a positive Unix timestamp");
    }

    signal.ticker = signal.ticker.and_then(|ticker| {
        let cleaned = ticker.trim().trim_start_matches('$').to_ascii_uppercase();
        (!cleaned.is_empty() && cleaned.len() <= 16).then_some(cleaned)
    });

    Ok(signal)
}

async fn handle_connection(
    stream: UnixStream,
    store: Arc<SignalStore>,
    config: Arc<Config>,
) -> Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break;
        }
        if bytes_read > MAX_LINE_BYTES {
            warn!(bytes_read, "discarding oversized local signal payload");
            continue;
        }

        let parsed = match serde_json::from_str::<TelegramSignal>(line.trim_end()) {
            Ok(signal) => signal,
            Err(error) => {
                warn!(%error, "discarding malformed signal JSON");
                continue;
            }
        };

        let signal = match validate_signal(parsed, &config) {
            Ok(signal) => signal,
            Err(error) => {
                warn!(%error, "discarding invalid Telegram signal envelope");
                continue;
            }
        };

        match store.ingest(&signal) {
            Ok(IngestOutcome::NewMint) => {
                info!(
                    source = %signal.source,
                    message_id = signal.message_id,
                    mint = %signal.mint,
                    ticker = signal.ticker.as_deref().unwrap_or(""),
                    "new Telegram mint recorded; live execution is intentionally disabled"
                );
            }
            Ok(IngestOutcome::DuplicateMint) => {
                info!(
                    source = %signal.source,
                    message_id = signal.message_id,
                    mint = %signal.mint,
                    "same mint already received from another message/channel; no second entry"
                );
            }
            Ok(IngestOutcome::DuplicateMessage) => {
                info!(
                    source = %signal.source,
                    message_id = signal.message_id,
                    "duplicate Telegram message ignored"
                );
            }
            Err(error) => error!(%error, "failed to persist Telegram signal"),
        }
    }

    Ok(())
}

fn prepare_socket(path: &str) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => {
            std::fs::remove_file(path)
                .with_context(|| format!("remove stale Unix socket at {path}"))?;
        }
        Ok(_) => bail!("refusing to replace non-socket filesystem entry at {path}"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("inspect socket path {path}")),
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    if env::var("LIVE_TRADING_ENABLED")
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        bail!(
            "LIVE_TRADING_ENABLED=true is not supported yet: the Telegram intake is wired, but the DEX-specific execution adapter has not been implemented or verified"
        );
    }

    let config = Arc::new(Config::from_env()?);
    let store = Arc::new(SignalStore::open(&config.database_path)?);
    prepare_socket(&config.socket_path)?;
    let listener = UnixListener::bind(&config.socket_path)
        .with_context(|| format!("bind Unix socket at {}", config.socket_path))?;
    std::fs::set_permissions(
        &config.socket_path,
        std::fs::Permissions::from_mode(0o600),
    )
    .with_context(|| format!("restrict signal socket permissions at {}", config.socket_path))?;

    info!(
        socket = %config.socket_path,
        database = %config.database_path,
        allowed_channels = ?config.allowed_channels,
        "Telegram signal intake ready; observe-only mode"
    );

    loop {
        match listener.accept().await {
            Ok((stream, _address)) => {
                let store = Arc::clone(&store);
                let config = Arc::clone(&config);
                tokio::spawn(async move {
                    if let Err(error) = handle_connection(stream, store, config).await {
                        warn!(%error, "signal listener connection ended with an error");
                    }
                });
            }
            Err(error) => error!(%error, "failed to accept local signal connection"),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            socket_path: "/tmp/telsolsniper-test.sock".into(),
            database_path: ":memory:".into(),
            allowed_channels: ["soltrenchtrending", "solwhaletrending", "solearlytrending"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        }
    }

    fn signal(source: &str, message_id: i64) -> TelegramSignal {
        TelegramSignal {
            source: source.into(),
            message_id,
            mint: "So11111111111111111111111111111111111111112".into(),
            ticker: Some("$sol".into()),
            message_text: "CA: So11111111111111111111111111111111111111112".into(),
            telegram_date_unix_ms: None,
            received_at_unix_ms: 1_800_000_000_000,
        }
    }

    #[test]
    fn validates_allowed_source_and_normalizes_ticker() {
        let validated = validate_signal(signal("@SolTrenchTrending", 1), &test_config())
            .expect("valid signal");
        assert_eq!(validated.source, "soltrenchtrending");
        assert_eq!(validated.ticker.as_deref(), Some("SOL"));
    }

    #[test]
    fn rejects_unknown_channel_and_invalid_mint() {
        let config = test_config();
        assert!(validate_signal(signal("unrelated_channel", 1), &config).is_err());

        let mut malformed = signal("soltrenchtrending", 2);
        malformed.mint = "not-a-solana-address".into();
        assert!(validate_signal(malformed, &config).is_err());
    }

    #[test]
    fn persists_first_mint_once_across_channels() {
        let store = SignalStore::open(":memory:").expect("in-memory store");
        let first = signal("soltrenchtrending", 10);
        let second_channel = signal("solwhaletrending", 20);

        assert_eq!(store.ingest(&first).unwrap(), IngestOutcome::NewMint);
        assert_eq!(
            store.ingest(&second_channel).unwrap(),
            IngestOutcome::DuplicateMint
        );
        assert_eq!(
            store.ingest(&first).unwrap(),
            IngestOutcome::DuplicateMessage
        );
    }

    #[test]
    fn keeps_every_mint_from_one_leaderboard_message() {
        let store = SignalStore::open(":memory:").expect("in-memory store");
        let first = signal("soltrenchtrending", 30);

        let mut second_mint = signal("soltrenchtrending", 30);
        second_mint.mint = "2StNe98imN7M6K341a7a4xvc8eNTvLCde5EugAKfLoat".into();

        assert_eq!(store.ingest(&first).unwrap(), IngestOutcome::NewMint);
        assert_eq!(store.ingest(&second_mint).unwrap(), IngestOutcome::NewMint);
        assert_eq!(
            store.ingest(&second_mint).unwrap(),
            IngestOutcome::DuplicateMessage
        );
    }

    #[test]
    fn migrates_legacy_single_mint_schema() {
        let folder = std::env::temp_dir().join(format!(
            "telsolsniper-migration-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("signals.sqlite");
        let path_str = path.to_str().unwrap();

        // Build the pre-migration schema and one legacy row.
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    r#"
                    CREATE TABLE telegram_messages (
                        source TEXT NOT NULL,
                        message_id INTEGER NOT NULL,
                        mint TEXT NOT NULL,
                        ticker TEXT,
                        message_text TEXT NOT NULL,
                        telegram_date_unix_ms INTEGER,
                        received_at_unix_ms INTEGER NOT NULL,
                        outcome TEXT NOT NULL,
                        PRIMARY KEY (source, message_id)
                    );
                    INSERT INTO telegram_messages
                        (source, message_id, mint, message_text, telegram_date_unix_ms, received_at_unix_ms, outcome)
                    VALUES ('soltrenchtrending', 5, 'So11111111111111111111111111111111111111112', 'legacy post', NULL, 1, 'new_mint');
                    "#,
                )
                .unwrap();
        }

        let store = SignalStore::open(path_str).expect("open legacy database");
        let mut second_mint = signal("soltrenchtrending", 5);
        second_mint.mint = "2StNe98imN7M6K341a7a4xvc8eNTvLCde5EugAKfLoat".into();

        // Legacy row survived the migration, and the same message can now hold
        // a second mint.
        assert_eq!(
            store.ingest(&second_mint).unwrap(),
            IngestOutcome::NewMint
        );

        let connection = store.connection.lock().unwrap();
        let rows: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM telegram_messages WHERE source = 'soltrenchtrending' AND message_id = 5",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows, 2);

        std::fs::remove_dir_all(&folder).ok();
    }
}

//! Durable order journal.
//!
//! The rule: an order is written to disk *before* it is submitted, so a
//! crash can never leave a transaction on chain that we have no record of.
//! After submit we store the signature; a reconciler then resolves each
//! entry from the chain: confirmed / failed / unknown.
//!
//! States:
//!   intended  — written, not yet sent (no signature)
//!   submitted — sent, signature known, not yet reconciled
//!   confirmed — landed successfully; lamports/tokens filled in from chain
//!   failed    — landed with an error, or rejected before send
//!   unknown   — cannot be resolved; requires operator review. Any trade
//!               with an unknown order must not be considered settled.

use anyhow::Result;
use rusqlite::{params, Connection};

pub const STATE_INTENDED: &str = "intended";
pub const STATE_SUBMITTED: &str = "submitted";
pub const STATE_CONFIRMED: &str = "confirmed";
pub const STATE_FAILED: &str = "failed";
pub const STATE_UNKNOWN: &str = "unknown";

#[derive(Debug, Clone)]
pub struct JournalOrder {
    pub id: i64,
    pub client_order_id: String,
    pub trade_id: Option<i64>,
    pub mint: String,
    pub side: String,
    pub kind: String,
    pub signature: Option<String>,
    pub state: String,
    pub lamports: i64,
    pub tokens: f64,
    pub created_ms: i64,
}

pub fn init(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS order_journal (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            client_order_id TEXT NOT NULL UNIQUE,
            trade_id INTEGER,
            mint TEXT NOT NULL,
            side TEXT NOT NULL,          -- buy | sell
            kind TEXT NOT NULL,          -- entry | tp1 | tp2 | moon_bag | stop | graduated
            signature TEXT,
            state TEXT NOT NULL,
            lamports INTEGER NOT NULL DEFAULT 0,
            tokens REAL NOT NULL DEFAULT 0,
            error TEXT,
            created_ms INTEGER NOT NULL,
            updated_ms INTEGER NOT NULL,
            reconciled_ms INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_journal_state ON order_journal(state);
        CREATE INDEX IF NOT EXISTS idx_journal_mint ON order_journal(mint);
        "#,
    )?;
    Ok(())
}

/// Write the intent before submitting. Returns the journal row id.
pub fn record_intent(
    connection: &Connection,
    client_order_id: &str,
    trade_id: Option<i64>,
    mint: &str,
    side: &str,
    kind: &str,
    expected_lamports: i64,
    expected_tokens: f64,
    now_ms: i64,
) -> Result<i64> {
    connection.execute(
        r#"INSERT INTO order_journal
             (client_order_id, trade_id, mint, side, kind, state, lamports, tokens, created_ms, updated_ms)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)"#,
        params![
            client_order_id,
            trade_id,
            mint,
            side,
            kind,
            STATE_INTENDED,
            expected_lamports,
            expected_tokens,
            now_ms
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

pub fn mark_submitted(connection: &Connection, id: i64, signature: &str, now_ms: i64) -> Result<()> {
    connection.execute(
        "UPDATE order_journal SET signature = ?2, state = ?3, updated_ms = ?4 WHERE id = ?1",
        params![id, signature, STATE_SUBMITTED, now_ms],
    )?;
    Ok(())
}

pub fn mark_confirmed(
    connection: &Connection,
    id: i64,
    lamports: i64,
    tokens: f64,
    now_ms: i64,
) -> Result<()> {
    connection.execute(
        r#"UPDATE order_journal
             SET state = ?2, lamports = ?3, tokens = ?4, updated_ms = ?5, reconciled_ms = ?5
           WHERE id = ?1"#,
        params![id, STATE_CONFIRMED, lamports, tokens, now_ms],
    )?;
    Ok(())
}

pub fn mark_failed(connection: &Connection, id: i64, error: &str, now_ms: i64) -> Result<()> {
    connection.execute(
        r#"UPDATE order_journal
             SET state = ?2, error = ?3, updated_ms = ?4, reconciled_ms = ?4
           WHERE id = ?1"#,
        params![id, STATE_FAILED, error, now_ms],
    )?;
    Ok(())
}

pub fn mark_unknown(connection: &Connection, id: i64, note: &str, now_ms: i64) -> Result<()> {
    connection.execute(
        r#"UPDATE order_journal
             SET state = ?2, error = ?3, updated_ms = ?4, reconciled_ms = ?4
           WHERE id = ?1"#,
        params![id, STATE_UNKNOWN, note, now_ms],
    )?;
    Ok(())
}

/// Orders that still need reconciliation (older than `min_age_ms`).
pub fn unsettled(connection: &Connection, min_age_ms: i64, now_ms: i64) -> Result<Vec<JournalOrder>> {
    let mut stmt = connection.prepare(
        r#"SELECT id, client_order_id, trade_id, mint, side, kind, signature, state,
                  lamports, tokens, created_ms
           FROM order_journal
           WHERE state IN (?1, ?2) AND created_ms <= ?3
           ORDER BY id"#,
    )?;
    let rows = stmt.query_map(params![STATE_INTENDED, STATE_SUBMITTED, now_ms - min_age_ms], |row| {
        Ok(JournalOrder {
            id: row.get(0)?,
            client_order_id: row.get(1)?,
            trade_id: row.get(2)?,
            mint: row.get(3)?,
            side: row.get(4)?,
            kind: row.get(5)?,
            signature: row.get(6)?,
            state: row.get(7)?,
            lamports: row.get(8)?,
            tokens: row.get(9)?,
            created_ms: row.get(10)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
}

/// True if any order for this mint is unresolved — its PnL must not be booked.
pub fn has_unresolved(connection: &Connection, mint: &str) -> Result<bool> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM order_journal WHERE mint = ?1 AND state IN (?2, ?3, ?4)",
        params![mint, STATE_INTENDED, STATE_SUBMITTED, STATE_UNKNOWN],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Confirmed proceeds (sells) or cost (buys) for a trade, in lamports.
pub fn confirmed_lamports(connection: &Connection, trade_id: i64, side: &str) -> Result<i64> {
    let total: i64 = connection.query_row(
        "SELECT COALESCE(SUM(lamports), 0) FROM order_journal WHERE trade_id = ?1 AND side = ?2 AND state = ?3",
        params![trade_id, side, STATE_CONFIRMED],
        |row| row.get(0),
    )?;
    Ok(total)
}

/// Journal rows for a mint, newest first (audit view).
pub fn history(connection: &Connection, mint: &str) -> Result<Vec<JournalOrder>> {
    let mut stmt = connection.prepare(
        r#"SELECT id, client_order_id, trade_id, mint, side, kind, signature, state,
                  lamports, tokens, created_ms
           FROM order_journal WHERE mint = ?1 ORDER BY id DESC"#,
    )?;
    let rows = stmt.query_map(params![mint], |row| {
        Ok(JournalOrder {
            id: row.get(0)?,
            client_order_id: row.get(1)?,
            trade_id: row.get(2)?,
            mint: row.get(3)?,
            side: row.get(4)?,
            kind: row.get(5)?,
            signature: row.get(6)?,
            state: row.get(7)?,
            lamports: row.get(8)?,
            tokens: row.get(9)?,
            created_ms: row.get(10)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        init(&connection).unwrap();
        connection
    }

    #[test]
    fn intent_is_durable_before_submit() {
        let connection = db();
        let id = record_intent(&connection, "c1", None, "mintA", "buy", "entry", 1_000_000, 100.0, 0)
            .unwrap();
        let order = &unsettled(&connection, 0, 1_000).unwrap()[0];
        assert_eq!(order.state, STATE_INTENDED);
        assert_eq!(order.lamports, 1_000_000);
        assert!(has_unresolved(&connection, "mintA").unwrap());
        assert_eq!(id, order.id);
    }

    #[test]
    fn confirmed_orders_count_only_when_confirmed() {
        let connection = db();
        let id = record_intent(&connection, "c1", Some(7), "mintA", "sell", "tp1", 0, 0.0, 0).unwrap();
        mark_submitted(&connection, id, "sig1", 1).unwrap();
        assert_eq!(confirmed_lamports(&connection, 7, "sell").unwrap(), 0);
        mark_confirmed(&connection, id, 500_000, 10.0, 2).unwrap();
        assert_eq!(confirmed_lamports(&connection, 7, "sell").unwrap(), 500_000);
    }

    #[test]
    fn unknown_blocks_settlement() {
        let connection = db();
        let id = record_intent(&connection, "c1", Some(7), "mintB", "sell", "stop", 0, 0.0, 0).unwrap();
        mark_unknown(&connection, id, "tx not found", 1).unwrap();
        assert!(has_unresolved(&connection, "mintB").unwrap());
        assert!(!unsettled(&connection, 0, 10_000).unwrap().iter().any(|o| o.id == id));
    }

    #[test]
    fn duplicate_client_order_id_rejected() {
        let connection = db();
        record_intent(&connection, "dup", None, "mintC", "buy", "entry", 1, 1.0, 0).unwrap();
        assert!(record_intent(&connection, "dup", None, "mintC", "buy", "entry", 1, 1.0, 0).is_err());
    }
}

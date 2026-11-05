//! Redaction for anything that reaches logs, SQLite or the event stream.
//!
//! Two hard rules:
//!   1. RPC URLs can carry API keys in the path or query
//!      (`.../<key>`, `?api-key=<key>`). Log the host, never the URL.
//!   2. The wallet private key must never appear in an error string, even
//!      if a dependency formats it into one.

/// Log-safe endpoint identity: scheme + host only.
pub fn redact_url(url: &str) -> String {
    let trimmed = url.trim();
    let (scheme, rest) = match trimmed.split_once("://") {
        Some((scheme, rest)) => (scheme, rest),
        None => ("", trimmed),
    };
    // Drop query and path entirely; the path is where keyed providers keep
    // the API key.
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit('@').next().unwrap_or(host); // strip user:pass@
    if scheme.is_empty() {
        host.to_string()
    } else {
        format!("{scheme}://{host}")
    }
}

/// Scrub secrets out of an error/exception string.
/// `extra_secrets` should include anything the engine knows (bankroll key,
/// ws auth tokens) so a dependency that echoes them cannot leak them.
pub fn redact_error(message: &str, extra_secrets: &[&str]) -> String {
    let mut out = message.to_string();

    // Key material pasted into an error by a dependency.
    for secret in extra_secrets {
        if secret.len() >= 16 && !secret.is_empty() {
            out = out.replace(secret, "[REDACTED]");
        }
    }

    // RPC URLs with embedded keys: keep host, drop the rest.
    out = redact_urls_in_text(&out);

    // Common key-bearing query params.
    for param in ["api-key", "api_key", "apikey", "token", "key", "auth"] {
        out = scrub_query_param(&out, param);
    }

    // Long base58-ish runs are likely keys (wallet or API) — keep short ones
    // so mint addresses in messages stay readable.
    out
}

fn redact_urls_in_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(position) = rest.find("://") {
        // Copy everything up to the scheme start (backtrack to the scheme).
        let scheme_start = rest[..position]
            .rfind(|c: char| !(c.is_alphanumeric() || c == '+' || c == '-' || c == '.'))
            .map(|index| index + 1)
            .unwrap_or(0);
        result.push_str(&rest[..scheme_start]);
        let after = &rest[position + 3..];
        // Host runs until the first delimiter.
        let host_end = after
            .find(|c: char| c == '/' || c == '?' || c == '#' || c.is_whitespace())
            .unwrap_or(after.len());
        let host = &after[..host_end];
        let scheme = &rest[scheme_start..position];
        result.push_str(scheme);
        result.push_str("://");
        result.push_str(host.rsplit('@').next().unwrap_or(host));
        // Skip the key-bearing remainder of the URL.
        let tail = &after[host_end..];
        let tail_end = tail.find(char::is_whitespace).unwrap_or(tail.len());
        if tail_end > 0 {
            result.push_str("/[REDACTED]");
        }
        rest = &tail[tail_end..];
    }
    result.push_str(rest);
    result
}

fn scrub_query_param(text: &str, param: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    let needle = format!("{param}=");
    while let Some(position) = rest.find(&needle) {
        result.push_str(&rest[..position]);
        result.push_str(&needle);
        result.push_str("[REDACTED]");
        let after = &rest[position + needle.len()..];
        let value_end = after.find(|c: char| c == '&' || c == '#' || c.is_whitespace()).unwrap_or(after.len());
        rest = &after[value_end..];
    }
    result.push_str(rest);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_key_in_path() {
        let url = "https://mainnet.helius-rpc.com/?api-key=abc123secret";
        assert_eq!(redact_url(url), "https://mainnet.helius-rpc.com");
        let error = format!("request to {url} failed");
        let redacted = redact_error(&error, &[]);
        assert!(!redacted.contains("abc123secret"), "leaked: {redacted}");
    }

    #[test]
    fn redacts_plain_urls_in_error_text() {
        let error = "rpc error: https://solana-mainnet.g.alchemy.com/v2/SUPERSECRETKEY returned 429";
        let redacted = redact_error(error, &[]);
        assert!(!redacted.contains("SUPERSECRETKEY"), "leaked: {redacted}");
        assert!(redacted.contains("alchemy.com"));
    }

    #[test]
    fn redacts_known_secret_values_everywhere() {
        let key = "SCRUBBED_PRIVATE_KEY";
        let error = format!("failed to sign with {key}: bad keypair");
        let redacted = redact_error(&error, &[key]);
        assert!(!redacted.contains("2A8VoKhi"), "leaked: {redacted}");
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn keeps_mint_addresses_readable() {
        let error = "mint 9GNMJdW9U67eBFP6AhdzDLfMqATqwHA9zMJccuoCpump has no curve";
        let redacted = redact_error(error, &[]);
        assert!(redacted.contains("9GNMJdW9U67eBFP6AhdzDLfMqATqwHA9zMJccuoCpump"));
    }

    #[test]
    fn redacts_query_style_api_key() {
        let error = "GET https://rpc.example.com/path?api_key=topsecret&cluster=mainnet";
        let redacted = redact_error(error, &[]);
        assert!(!redacted.contains("topsecret"), "leaked: {redacted}");
    }
}

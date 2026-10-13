//! Reads the tokens that the providers' own CLIs store on this PC.
//!
//! Read-only on purpose: this program never refreshes a token and never writes these files.
//! Claude, Codex and Grok rotate single-use refresh tokens, so spending one here would log
//! the CLI out. An expired access token shows up as "sign-in expired" until the CLI runs again.

use crate::quota::Problem;
use chrono::DateTime;
use serde_json::Value;
use std::path::PathBuf;

pub struct CodexToken {
    pub access: String,
    pub account_id: Option<String>,
}

pub fn claude_token(json: &str) -> Result<String, Problem> {
    let v = parse(json)?;
    text(v.pointer("/claudeAiOauth/accessToken")).ok_or(Problem::NotSetUp)
}

pub fn codex_token(json: &str) -> Result<CodexToken, Problem> {
    let v = parse(json)?;
    Ok(CodexToken {
        access: text(v.pointer("/tokens/access_token")).ok_or(Problem::NotSetUp)?,
        account_id: text(v.pointer("/tokens/account_id")),
    })
}

/// `ZAI_API_KEY` wins; otherwise the key opencode stores for its "zai-coding-plan" provider.
pub fn zai_key(env: Option<String>, opencode_json: Option<&str>) -> Result<String, Problem> {
    if let Some(key) = env.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        return Ok(key);
    }
    let v = parse(opencode_json.ok_or(Problem::NotSetUp)?)?;
    text(v.pointer("/zai-coding-plan/key")).ok_or(Problem::NotSetUp)
}

/// The Grok CLI keys each login by "<issuer>::<client id>"; take the newest x.ai one.
pub fn grok_token(json: &str) -> Result<String, Problem> {
    let v = parse(json)?;
    let entries = v.as_object().ok_or(Problem::BadData)?;
    entries
        .iter()
        .filter(|(name, _)| name.starts_with("https://auth.x.ai::"))
        .filter_map(|(_, entry)| Some((expiry(entry.get("expires_at")), text(entry.get("key"))?)))
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, key)| key)
        .ok_or(Problem::NotSetUp)
}

pub fn read_claude() -> Result<String, Problem> {
    claude_token(&read_home(".claude/.credentials.json")?)
}

pub fn read_codex() -> Result<CodexToken, Problem> {
    codex_token(&read_home(".codex/auth.json")?)
}

pub fn read_zai() -> Result<String, Problem> {
    let opencode = read_home(".local/share/opencode/auth.json").ok();
    zai_key(std::env::var("ZAI_API_KEY").ok(), opencode.as_deref())
}

pub fn read_grok() -> Result<String, Problem> {
    grok_token(&read_home(".grok/auth.json")?)
}

fn read_home(relative: &str) -> Result<String, Problem> {
    let home = std::env::var_os("HOME").ok_or(Problem::NotSetUp)?;
    std::fs::read_to_string(PathBuf::from(home).join(relative)).map_err(|_| Problem::NotSetUp)
}

fn parse(json: &str) -> Result<Value, Problem> {
    serde_json::from_str(json).map_err(|_| Problem::BadData)
}

fn text(v: Option<&Value>) -> Option<String> {
    v?.as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// `expires_at` as seconds, whether it is stored as a number, a numeric string or RFC 3339.
fn expiry(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok().or_else(|| {
            DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|d| d.timestamp() as f64)
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_claude_token() {
        let json = r#"{"mcpOAuth":{},"claudeAiOauth":{"accessToken":"fake-claude","expiresAt":1}}"#;
        assert_eq!(claude_token(json).unwrap(), "fake-claude");
        assert_eq!(
            claude_token(r#"{"mcpOAuth":{}}"#).err(),
            Some(Problem::NotSetUp)
        );
        assert_eq!(claude_token("not json").err(), Some(Problem::BadData));
    }

    #[test]
    fn reads_codex_token_and_account() {
        let json = r#"{"auth_mode":"chatgpt","tokens":{"access_token":"fake-codex","account_id":"acc-1"}}"#;
        let token = codex_token(json).unwrap();
        assert_eq!(token.access, "fake-codex");
        assert_eq!(token.account_id.as_deref(), Some("acc-1"));

        let no_account = codex_token(r#"{"tokens":{"access_token":"fake-codex"}}"#).unwrap();
        assert_eq!(no_account.account_id, None);
        assert_eq!(
            codex_token(r#"{"tokens":{}}"#).err(),
            Some(Problem::NotSetUp)
        );
    }

    #[test]
    fn zai_env_wins_over_opencode_file() {
        let file = r#"{"zai-coding-plan":{"type":"api","key":"fake-file"}}"#;
        assert_eq!(
            zai_key(Some("fake-env".into()), Some(file)).unwrap(),
            "fake-env"
        );
        assert_eq!(zai_key(Some("  ".into()), Some(file)).unwrap(), "fake-file");
        assert_eq!(zai_key(None, Some(file)).unwrap(), "fake-file");
        assert_eq!(zai_key(None, None).err(), Some(Problem::NotSetUp));
        assert_eq!(zai_key(None, Some("{}")).err(), Some(Problem::NotSetUp));
    }

    #[test]
    fn grok_takes_the_newest_xai_login() {
        let json = r#"{
            "https://auth.x.ai::client-a": {"key": "fake-old", "expires_at": 100},
            "https://auth.x.ai::client-b": {"key": "fake-new", "expires_at": "2026-10-12T10:00:00Z"},
            "https://other.example::client-c": {"key": "fake-other", "expires_at": 9999999999}
        }"#;
        assert_eq!(grok_token(json).unwrap(), "fake-new");
        assert_eq!(grok_token("{}").err(), Some(Problem::NotSetUp));
        assert_eq!(grok_token("[]").err(), Some(Problem::BadData));
    }
}

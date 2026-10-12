use super::{get_body, number, parse_json, used_percent};
use crate::quota::{Problem, Window};
use crate::tokens::CodexToken;
use chrono::DateTime;
use reqwest::blocking::Client;
use serde_json::Value;

const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

pub fn fetch(client: &Client, token: &CodexToken) -> Result<Vec<Window>, Problem> {
    let mut request = client.get(USAGE_URL).bearer_auth(&token.access);
    if let Some(account_id) = &token.account_id {
        request = request.header("ChatGPT-Account-Id", account_id);
    }
    parse(&get_body(request)?)
}

/// Reads the primary and secondary windows. A malformed window is skipped, not fatal.
pub fn parse(body: &str) -> Result<Vec<Window>, Problem> {
    let v = parse_json(body)?;
    let mut windows = Vec::new();
    // Payloads without `limit_window_seconds` are the old shape: primary is 5-hour, secondary weekly.
    for (key, legacy_label) in [("primary_window", "5H"), ("secondary_window", "WK")] {
        let Some(w) = v
            .get("rate_limit")
            .and_then(|r| r.get(key))
            .and_then(Value::as_object)
        else {
            continue;
        };
        let label = match w.get("limit_window_seconds") {
            None | Some(Value::Null) => legacy_label,
            seconds => match number(seconds).map(|s| s as i64) {
                Some(18_000) => "5H",
                Some(604_800) => "WK",
                Some(86_400) => "DAY",
                Some(2_592_000) => "MO",
                _ => continue,
            },
        };
        let Some(used) = used_percent(w.get("used_percent")) else {
            continue;
        };
        let resets_at = match number(w.get("reset_at")) {
            None => None,
            Some(seconds) => match DateTime::from_timestamp(seconds as i64, 0) {
                Some(instant) => Some(instant),
                None => continue,
            },
        };
        windows.push(Window {
            label,
            used_percent: used,
            resets_at,
        });
    }
    Ok(windows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn reads_windows_by_length() {
        let windows = parse(include_str!("../../tests/fixtures/codex/usage.json")).unwrap();
        assert_eq!(
            windows,
            vec![
                Window {
                    label: "5H",
                    used_percent: 12.5,
                    resets_at: Some(Utc.with_ymd_and_hms(2026, 10, 12, 18, 0, 0).unwrap()),
                },
                Window {
                    label: "WK",
                    used_percent: 45.0,
                    resets_at: Some(Utc.with_ymd_and_hms(2026, 10, 17, 18, 0, 0).unwrap()),
                },
            ]
        );
    }

    #[test]
    fn legacy_payload_uses_primary_and_secondary_positions() {
        let body = r#"{"rate_limit": {
            "primary_window": {"used_percent": 10, "reset_at": null},
            "secondary_window": {"used_percent": 20}
        }}"#;
        let labels: Vec<_> = parse(body).unwrap().iter().map(|w| w.label).collect();
        assert_eq!(labels, ["5H", "WK"]);
    }

    #[test]
    fn skips_malformed_and_unknown_windows() {
        let body = r#"{"rate_limit": {
            "primary_window": {"used_percent": "lots", "limit_window_seconds": 18000},
            "secondary_window": {"used_percent": 20, "limit_window_seconds": 1234}
        }}"#;
        assert_eq!(parse(body).unwrap(), vec![]);
        assert_eq!(parse(r#"{"plan_type": "plus"}"#).unwrap(), vec![]);
    }
}

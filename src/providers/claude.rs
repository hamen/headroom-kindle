use super::{get_body, parse_json, rfc3339, used_percent};
use crate::quota::{Problem, Window};
use reqwest::blocking::Client;
use serde_json::Value;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
/// The Claude Code identity Headroom pins (ClaudeCodeIdentity.kt). Bump it if the endpoint starts rejecting it.
const USER_AGENT: &str = "claude-cli/2.1.296";

pub fn fetch(client: &Client, token: &str) -> Result<Vec<Window>, Problem> {
    let request = client
        .get(USAGE_URL)
        .bearer_auth(token)
        .header("anthropic-beta", "claude-code-20250219,oauth-2025-04-20")
        .header("User-Agent", USER_AGENT)
        .header("x-app", "cli");
    parse(&get_body(request)?)
}

/// Reads the 5-hour and weekly windows. Per-model, extra-usage and credit rows are ignored.
pub fn parse(body: &str) -> Result<Vec<Window>, Problem> {
    let v = parse_json(body)?;
    let mut windows = Vec::new();
    for (key, label) in [("five_hour", "5H"), ("seven_day", "WK")] {
        let Some(w) = v.get(key).and_then(Value::as_object) else {
            continue;
        };
        if w.get("is_enabled") == Some(&Value::Bool(false)) {
            continue;
        }
        let Some(used) =
            used_percent(w.get("utilization")).or_else(|| used_percent(w.get("used_percent")))
        else {
            continue;
        };
        windows.push(Window {
            label,
            used_percent: used,
            resets_at: rfc3339(w.get("resets_at"))?,
        });
    }
    Ok(windows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn reads_five_hour_and_weekly() {
        let windows = parse(include_str!("../../tests/fixtures/claude/usage.json")).unwrap();
        assert_eq!(
            windows,
            vec![
                Window {
                    label: "5H",
                    used_percent: 27.5,
                    resets_at: Some(Utc.with_ymd_and_hms(2026, 10, 12, 17, 0, 0).unwrap()),
                },
                Window {
                    label: "WK",
                    used_percent: 61.0,
                    resets_at: Some(Utc.with_ymd_and_hms(2026, 10, 19, 9, 0, 0).unwrap()),
                },
            ]
        );
    }

    #[test]
    fn skips_null_disabled_and_unusable_windows() {
        let body = r#"{"five_hour": null, "seven_day": {"is_enabled": false, "utilization": 5}}"#;
        assert_eq!(parse(body).unwrap(), vec![]);
        let negative = r#"{"five_hour": {"utilization": -3, "resets_at": null}}"#;
        assert_eq!(parse(negative).unwrap(), vec![]);
    }

    #[test]
    fn falls_back_to_used_percent() {
        let body =
            r#"{"five_hour": {"utilization": null, "used_percent": 12.5, "resets_at": null}}"#;
        let windows = parse(body).unwrap();
        assert_eq!(windows[0].used_percent, 12.5);
        assert_eq!(windows[0].resets_at, None);
    }

    #[test]
    fn empty_object_has_no_windows_and_bad_dates_are_bad_data() {
        assert_eq!(parse("{}").unwrap(), vec![]);
        let body = r#"{"five_hour": {"utilization": 1, "resets_at": "yesterday"}}"#;
        assert_eq!(parse(body), Err(Problem::BadData));
        assert_eq!(parse("<html>"), Err(Problem::BadData));
    }
}

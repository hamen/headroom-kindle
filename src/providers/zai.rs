use super::{get_body, number, parse_json, used_percent};
use crate::quota::{Problem, Window};
use chrono::DateTime;
use reqwest::blocking::Client;
use serde_json::Value;

const QUOTA_URL: &str = "https://api.z.ai/api/monitor/usage/quota/limit";

pub fn fetch(client: &Client, key: &str) -> Result<Vec<Window>, Problem> {
    parse(&get_body(client.get(QUOTA_URL).bearer_auth(key))?)
}

/// Rows are matched by `unit` and `number`, never by `type`: (3, 5) is the 5-hour window,
/// (6, 1) the weekly one. Every other row is ignored. No usable row at all is bad data.
pub fn parse(body: &str) -> Result<Vec<Window>, Problem> {
    let v = parse_json(body)?;
    let rows = v
        .get("data")
        .and_then(|d| d.get("limits"))
        .and_then(Value::as_array)
        .ok_or(Problem::BadData)?;
    let mut windows = Vec::new();
    for row in rows {
        let unit = number(row.get("unit")).map(|u| u as i64);
        let count = number(row.get("number")).map(|n| n as i64);
        let label = match (unit, count) {
            (Some(3), Some(5)) => "5H",
            (Some(6), Some(1)) => "WK",
            _ => continue,
        };
        let used = match (number(row.get("currentValue")), number(row.get("usage"))) {
            (Some(current), Some(limit)) if limit > 0.0 && current >= 0.0 => {
                current / limit * 100.0
            }
            _ => match used_percent(row.get("percentage")) {
                Some(percent) => percent,
                None => continue,
            },
        };
        let resets_at = number(row.get("nextResetTime"))
            .filter(|ms| *ms >= 1e12)
            .and_then(|ms| DateTime::from_timestamp_millis(ms as i64));
        windows.push(Window {
            label,
            used_percent: used,
            resets_at,
        });
    }
    if windows.is_empty() {
        return Err(Problem::BadData);
    }
    Ok(windows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn reads_five_hour_and_weekly_rows() {
        let windows = parse(include_str!("../../tests/fixtures/zai/quota_limit.json")).unwrap();
        assert_eq!(
            windows,
            vec![
                Window {
                    label: "5H",
                    used_percent: 37.5,
                    resets_at: Some(Utc.with_ymd_and_hms(2026, 10, 12, 18, 0, 0).unwrap()),
                },
                Window {
                    label: "WK",
                    used_percent: 75.0,
                    resets_at: Some(Utc.with_ymd_and_hms(2026, 10, 17, 18, 0, 0).unwrap()),
                },
            ]
        );
    }

    #[test]
    fn falls_back_to_percentage() {
        let body = r#"{"data": {"limits": [
            {"unit": 3, "number": 5, "usage": 0, "currentValue": 5, "percentage": 12.5, "nextResetTime": 0},
            {"unit": 6, "number": 1, "percentage": "lots"}
        ]}}"#;
        let windows = parse(body).unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].used_percent, 12.5);
        assert_eq!(windows[0].resets_at, None);
    }

    #[test]
    fn no_known_row_is_bad_data() {
        let body = r#"{"code": 200, "success": true, "data": {"limits": [{"unit": 5, "number": 1, "percentage": 1}]}}"#;
        assert_eq!(parse(body), Err(Problem::BadData));
        assert_eq!(
            parse(r#"{"code": 1001, "success": false, "msg": "bad key"}"#),
            Err(Problem::BadData)
        );
    }
}

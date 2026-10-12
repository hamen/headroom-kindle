use super::{get_body, number, parse_json, rfc3339};
use crate::quota::{Problem, Window};
use reqwest::blocking::{Client, RequestBuilder};
use serde_json::Value;

const WEEKLY_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing?format=credits";
const MONTHLY_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing";

/// The weekly credit pool first; the monthly budget only when the weekly answer has no usable window.
/// An HTTP or network error on either call is reported as it is, with no fallback.
pub fn fetch(client: &Client, token: &str) -> Result<Vec<Window>, Problem> {
    if let Some(window) = parse_weekly(&get_body(request(client, WEEKLY_URL, token))?)? {
        return Ok(vec![window]);
    }
    Ok(vec![parse_monthly(&get_body(request(
        client,
        MONTHLY_URL,
        token,
    ))?)?])
}

fn request(client: &Client, url: &str, token: &str) -> RequestBuilder {
    client
        .get(url)
        .bearer_auth(token)
        .header("X-XAI-Token-Auth", "xai-grok-cli")
}

/// `Ok(None)` means no usable weekly window, so the monthly budget applies.
pub fn parse_weekly(body: &str) -> Result<Option<Window>, Problem> {
    let v = parse_json(body)?;
    let v = unwrap_config(&v);
    let Some(period) = v.get("currentPeriod").filter(|p| p.is_object()) else {
        return Ok(None);
    };
    // Right after a reset the proxy leaves the percent out but keeps the period: nothing used yet.
    let used = match v.get("creditUsagePercent") {
        None | Some(Value::Null) => 0.0,
        percent => match number(percent) {
            Some(p) if (0.0..=100.0).contains(&p) => p,
            _ => return Ok(None),
        },
    };
    Ok(Some(Window {
        label: "WK",
        used_percent: used,
        resets_at: rfc3339(period.get("end"))?,
    }))
}

/// The monthly budget, in cents, sent as a number, a numeric string or `{"val": n}`.
pub fn parse_monthly(body: &str) -> Result<Window, Problem> {
    let v = parse_json(body)?;
    let v = unwrap_config(&v);
    let limit = cents(v.get("monthlyLimit"))
        .filter(|l| *l > 0.0)
        .ok_or(Problem::BadData)?;
    let used = cents(v.get("used"))
        .filter(|u| *u >= 0.0)
        .ok_or(Problem::BadData)?;
    Ok(Window {
        label: "MO",
        used_percent: used / limit * 100.0,
        resets_at: rfc3339(v.get("billingPeriodEnd"))?,
    })
}

fn unwrap_config(v: &Value) -> &Value {
    v.get("config").filter(|c| c.is_object()).unwrap_or(v)
}

fn cents(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Object(o) => number(o.get("val")),
        other => number(Some(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn reads_weekly_credits() {
        let window = parse_weekly(include_str!(
            "../../tests/fixtures/grok/billing_weekly.json"
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            window,
            Window {
                label: "WK",
                used_percent: 38.5,
                resets_at: Some(Utc.with_ymd_and_hms(2026, 10, 15, 0, 0, 0).unwrap()),
            }
        );
    }

    #[test]
    fn weekly_without_wrapper_and_after_reset() {
        let body = r#"{"currentPeriod": {"type": "USAGE_PERIOD_TYPE_WEEKLY", "end": "2026-10-15T00:00:00Z"}}"#;
        assert_eq!(parse_weekly(body).unwrap().unwrap().used_percent, 0.0);
    }

    #[test]
    fn weekly_falls_through_to_monthly() {
        let no_period = r#"{"config": {"currentPeriod": null, "isUnifiedBillingUser": true}}"#;
        assert_eq!(parse_weekly(no_period).unwrap(), None);
        let out_of_range =
            r#"{"creditUsagePercent": 140, "currentPeriod": {"end": "2026-10-15T00:00:00Z"}}"#;
        assert_eq!(parse_weekly(out_of_range).unwrap(), None);
    }

    #[test]
    fn reads_monthly_budget_in_every_number_shape() {
        let window = parse_monthly(include_str!(
            "../../tests/fixtures/grok/billing_monthly.json"
        ))
        .unwrap();
        assert_eq!(window.label, "MO");
        assert_eq!(window.used_percent, 25.0);
        assert_eq!(
            window.resets_at,
            Some(Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap())
        );
        let plain = r#"{"monthlyLimit": 2000, "used": "500", "billingPeriodEnd": null}"#;
        assert_eq!(parse_monthly(plain).unwrap().used_percent, 25.0);
    }

    #[test]
    fn monthly_without_a_limit_is_bad_data() {
        assert_eq!(
            parse_monthly(r#"{"monthlyLimit": 0, "used": 10}"#),
            Err(Problem::BadData)
        );
        assert_eq!(parse_monthly("{}"), Err(Problem::BadData));
    }
}

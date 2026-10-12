//! Quota fetchers. Endpoints, headers and response fields follow the Headroom app
//! (github.com/rock3r/headroom, core/quota), which reads the same undocumented endpoints.

pub mod claude;
pub mod codex;
pub mod grok;
pub mod zai;

use crate::quota::{status_problem, Problem};
use chrono::{DateTime, Utc};
use reqwest::blocking::RequestBuilder;
use serde_json::Value;

/// Sends the request and returns the body of a 200 response.
/// Errors carry no request details, so a token can never end up in a log line.
fn get_body(request: RequestBuilder) -> Result<String, Problem> {
    let response = request
        .header("Accept", "application/json")
        .send()
        .map_err(|_| Problem::Offline)?;
    let code = response.status().as_u16();
    if code != 200 {
        return Err(status_problem(code));
    }
    response.text().map_err(|_| Problem::Offline)
}

fn parse_json(body: &str) -> Result<Value, Problem> {
    serde_json::from_str(body).map_err(|_| Problem::BadData)
}

/// A number sent as a JSON number or as a numeric string.
fn number(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// A usable "percent used": a number that is not negative.
fn used_percent(v: Option<&Value>) -> Option<f64> {
    number(v).filter(|p| *p >= 0.0)
}

/// An RFC 3339 timestamp. Absent or null means no reset time; anything else that does not parse is bad data.
fn rfc3339(v: Option<&Value>) -> Result<Option<DateTime<Utc>>, Problem> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => DateTime::parse_from_rfc3339(s)
            .map(|d| Some(d.with_timezone(&Utc)))
            .map_err(|_| Problem::BadData),
        Some(_) => Err(Problem::BadData),
    }
}

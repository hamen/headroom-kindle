use chrono::{DateTime, Duration, TimeZone, Utc};
use std::fmt::Display;

/// One quota window, as the provider reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    /// Short name of the window length: "5H", "WK", "DAY" or "MO".
    pub label: &'static str,
    /// Percent of the window already used, 0-100 (more than 100 when over the limit).
    pub used_percent: f64,
    pub resets_at: Option<DateTime<Utc>>,
}

/// Why a provider has no numbers to show.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Problem {
    NotSetUp,
    SignInExpired,
    NoAccess,
    RateLimited,
    Http(u16),
    Offline,
    BadData,
}

impl Problem {
    pub fn text(&self) -> String {
        match self {
            Problem::NotSetUp => "not set up".to_string(),
            Problem::SignInExpired => "sign-in expired".to_string(),
            Problem::NoAccess => "no access".to_string(),
            Problem::RateLimited => "rate limited".to_string(),
            Problem::Http(code) => format!("error {code}"),
            Problem::Offline => "offline".to_string(),
            Problem::BadData => "bad data".to_string(),
        }
    }
}

/// Maps a non-200 HTTP status to a problem, the same way Headroom does (FetchSupport.kt).
pub fn status_problem(code: u16) -> Problem {
    match code {
        401 => Problem::SignInExpired,
        403 => Problem::NoAccess,
        429 => Problem::RateLimited,
        other => Problem::Http(other),
    }
}

pub fn percent_left(window: &Window) -> i32 {
    (100.0 - window.used_percent).clamp(0.0, 100.0).round() as i32
}

/// The window with the least quota left; the first one on a tie.
pub fn tightest(windows: &[Window]) -> Option<&Window> {
    windows.iter().fold(None, |best, w| match best {
        Some(b) if b.used_percent >= w.used_percent => Some(b),
        _ => Some(w),
    })
}

/// "HH:MM" when the reset is less than a day away, "DDD HH:MM" after that, "--" when unknown.
pub fn format_reset<Tz: TimeZone>(now: &DateTime<Tz>, reset: Option<DateTime<Utc>>) -> String
where
    Tz::Offset: Display,
{
    let Some(reset) = reset else {
        return "--".to_string();
    };
    // The APIs send instants like 20:59:59.6; show them as the minute they round to.
    let local = (reset + Duration::seconds(30)).with_timezone(&now.timezone());
    if local.clone() - now.clone() < Duration::hours(24) {
        local.format("%H:%M").to_string()
    } else {
        local.format("%a %H:%M").to_string().to_uppercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn window(label: &'static str, used_percent: f64) -> Window {
        Window {
            label,
            used_percent,
            resets_at: None,
        }
    }

    #[test]
    fn maps_http_status() {
        assert_eq!(status_problem(401), Problem::SignInExpired);
        assert_eq!(status_problem(403), Problem::NoAccess);
        assert_eq!(status_problem(429), Problem::RateLimited);
        assert_eq!(status_problem(500), Problem::Http(500));
        assert_eq!(Problem::Http(503).text(), "error 503");
    }

    #[test]
    fn percent_left_is_clamped_and_rounded() {
        assert_eq!(percent_left(&window("5H", 27.4)), 73);
        assert_eq!(percent_left(&window("5H", 120.0)), 0);
        assert_eq!(percent_left(&window("5H", 0.0)), 100);
    }

    #[test]
    fn tightest_picks_the_least_left_and_the_first_on_a_tie() {
        let windows = [window("5H", 40.0), window("WK", 80.0), window("MO", 80.0)];
        assert_eq!(tightest(&windows).unwrap().label, "WK");
        assert_eq!(tightest(&[]), None);
    }

    #[test]
    fn formats_reset_time() {
        let rome = FixedOffset::east_opt(2 * 3600).unwrap();
        let now = rome.with_ymd_and_hms(2026, 10, 12, 20, 0, 0).unwrap();
        let soon = Utc.with_ymd_and_hms(2026, 10, 13, 7, 30, 0).unwrap();
        let later = Utc.with_ymd_and_hms(2026, 10, 19, 7, 0, 0).unwrap();
        assert_eq!(format_reset(&now, Some(soon)), "09:30");
        assert_eq!(format_reset(&now, Some(later)), "MON 09:00");
        assert_eq!(format_reset(&now, None), "--");
        let almost = Utc.with_ymd_and_hms(2026, 10, 17, 18, 59, 59).unwrap();
        assert_eq!(format_reset(&now, Some(almost)), "SAT 21:00");
    }
}

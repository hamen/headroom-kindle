mod kindle;
mod providers;
mod quota;
mod render;
mod tokens;

use chrono::{Local, Timelike};
use quota::percent_left;
use render::{Fonts, Tile};
use reqwest::blocking::Client;
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str = "usage: headroom-kindle [--no-push] [--full]
  (no flag)   fetch the quota, render it and draw it on the Kindle
  --no-push   only write dashboard.png in the current directory
  --full      clear the e-ink before drawing (flashes; done anyway on the first run of each hour)";

#[derive(Debug, PartialEq)]
struct Options {
    push: bool,
    full: bool,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        push: true,
        full: false,
    };
    for arg in args {
        match arg.as_str() {
            "--no-push" => options.push = false,
            "--full" => options.full = true,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(options)
}

/// The timer runs every 10 minutes, so exactly one run per hour lands in minutes 0-9:
/// that one clears the ghosting with a full refresh, the others redraw without flashing.
fn full_refresh(forced: bool, minute: u32) -> bool {
    forced || minute < 10
}

fn main() -> ExitCode {
    let options = match parse_args(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .expect("Errore client HTTP");

    // Checkerboard: black, white / white, black.
    let tiles = [
        Tile {
            name: "CLAUDE",
            quota: tokens::read_claude().and_then(|t| providers::claude::fetch(&client, &t)),
            invert: true,
        },
        Tile {
            name: "CODEX",
            quota: tokens::read_codex().and_then(|t| providers::codex::fetch(&client, &t)),
            invert: false,
        },
        Tile {
            name: "Z.AI",
            quota: tokens::read_zai().and_then(|k| providers::zai::fetch(&client, &k)),
            invert: false,
        },
        Tile {
            name: "GROK",
            quota: tokens::read_grok().and_then(|t| providers::grok::fetch(&client, &t)),
            invert: true,
        },
    ];
    for tile in &tiles {
        eprintln!("{}", summary(tile));
    }

    let now = Local::now();
    let png = render::png_bytes(&render::render(&now, &tiles, &Fonts::load()));

    if !options.push {
        std::fs::write("dashboard.png", &png).expect("Errore dashboard.png");
        return ExitCode::SUCCESS;
    }
    let full = full_refresh(options.full, now.minute());
    match kindle::push(&png, full) {
        Ok(()) => {
            eprintln!(
                "kindle: drawn ({} refresh)",
                if full { "full" } else { "plain" }
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("kindle: {e}");
            ExitCode::FAILURE
        }
    }
}

/// One line per provider: its numbers or its problem, never a token.
fn summary(tile: &Tile) -> String {
    match &tile.quota {
        Ok(windows) if windows.is_empty() => format!("{}: no data", tile.name),
        Ok(windows) => {
            let parts: Vec<String> = windows
                .iter()
                .map(|w| format!("{} {}% left", w.label, percent_left(w)))
                .collect();
            format!("{}: {}", tile.name, parts.join(", "))
        }
        Err(problem) => format!("{}: {}", tile.name, problem.text()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn parses_flags() {
        assert_eq!(
            parse_args(args(&[])).unwrap(),
            Options {
                push: true,
                full: false
            }
        );
        assert_eq!(
            parse_args(args(&["--no-push", "--full"])).unwrap(),
            Options {
                push: false,
                full: true
            }
        );
        assert!(parse_args(args(&["--bogus"])).is_err());
    }

    #[test]
    fn full_refresh_once_an_hour_or_when_asked() {
        assert!(full_refresh(false, 0));
        assert!(full_refresh(false, 9));
        assert!(!full_refresh(false, 10));
        assert!(!full_refresh(false, 50));
        assert!(full_refresh(true, 30));
    }
}

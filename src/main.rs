mod providers;
mod quota;
mod render;
mod tokens;

use chrono::Local;
use quota::percent_left;
use render::{Fonts, Tile};
use reqwest::blocking::Client;
use std::time::Duration;

fn main() {
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

    let image = render::render(&Local::now(), &tiles, &Fonts::load());
    image.save("dashboard.png").unwrap();
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

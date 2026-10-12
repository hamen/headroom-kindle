use crate::quota::{format_reset, percent_left, tightest, Problem, Window};
use chrono::{DateTime, Local, TimeZone};
use image::{Rgb, RgbImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_hollow_rect_mut, draw_text_mut};
use imageproc::rect::Rect;
use rusttype::{Font, Scale};
use std::fmt::Display;

pub const WIDTH: u32 = 758;
pub const HEIGHT: u32 = 1024;
const PADDING: u32 = 15;
const TILE_W: u32 = (WIDTH - PADDING * 3) / 2;
const TILE_H: u32 = 390;
const GRID_TOP: u32 = 150;
/// Left margin of the text inside a tile. Every line must fit in TILE_W - 2 * TEXT_X.
const TEXT_X: u32 = 25;
const SUB_INDENT: u32 = 5;

const BIG: f32 = 90.0;
const SUB: f32 = 30.0;
const LINE: f32 = 28.0;
const NAME: f32 = 35.0;

const BLACK: Rgb<u8> = Rgb([0, 0, 0]);
const WHITE: Rgb<u8> = Rgb([255, 255, 255]);

pub struct Fonts {
    bold: Font<'static>,
    light: Font<'static>,
}

impl Fonts {
    pub fn load() -> Fonts {
        let bold = std::fs::read("assets/font.ttf").expect("Errore font.ttf");
        let light = std::fs::read("assets/font_light.ttf").expect("Errore font_light.ttf");
        Fonts {
            bold: Font::try_from_vec(bold).unwrap(),
            light: Font::try_from_vec(light).unwrap(),
        }
    }
}

/// One provider tile: its name, its windows or the reason it has none, and its colours.
pub struct Tile {
    pub name: &'static str,
    pub quota: Result<Vec<Window>, Problem>,
    /// Black tile with white text.
    pub invert: bool,
}

/// The text of a tile, worked out before drawing so the tests can measure it.
#[derive(Debug, PartialEq)]
pub struct TileText {
    pub big: String,
    pub sub: String,
    pub lines: Vec<String>,
}

pub fn tile_text<Tz: TimeZone>(now: &DateTime<Tz>, quota: &Result<Vec<Window>, Problem>) -> TileText
where
    Tz::Offset: Display,
{
    let empty = |sub: String| TileText {
        big: "--".to_string(),
        sub,
        lines: Vec::new(),
    };
    let windows = match quota {
        Err(problem) => return empty(problem.text()),
        Ok(windows) => windows,
    };
    let Some(tight) = tightest(windows) else {
        return empty("no data".to_string());
    };
    TileText {
        big: format!("{}%", percent_left(tight)),
        sub: format!("left · {}", tight.label),
        lines: windows
            .iter()
            .map(|w| {
                let reset = format_reset(now, w.resets_at);
                format!("{} {}% · {}", w.label, percent_left(w), reset)
            })
            .collect(),
    }
}

pub fn render(now: &DateTime<Local>, tiles: &[Tile], fonts: &Fonts) -> RgbImage {
    let mut image = RgbImage::from_pixel(WIDTH, HEIGHT, WHITE);

    // Big clock and date at the top: the time of this render.
    let time_str = now.format("%H:%M").to_string();
    let date_str = now.format("%A, %d %b").to_string();
    draw_text_mut(
        &mut image,
        BLACK,
        PADDING as i32,
        20,
        Scale::uniform(100.0),
        &fonts.bold,
        &time_str,
    );
    draw_text_mut(
        &mut image,
        BLACK,
        PADDING as i32 + 300,
        70,
        Scale::uniform(45.0),
        &fonts.light,
        &date_str,
    );

    // 2x2 grid, filled row by row.
    for (i, tile) in tiles.iter().enumerate() {
        let x = PADDING + (i as u32 % 2) * (TILE_W + PADDING);
        let y = GRID_TOP + (i as u32 / 2) * (TILE_H + PADDING);
        draw_tile(&mut image, fonts, x, y, tile, &tile_text(now, &tile.quota));
    }

    draw_text_mut(
        &mut image,
        BLACK,
        PADDING as i32,
        HEIGHT as i32 - 40,
        Scale::uniform(25.0),
        &fonts.light,
        "Headroom Tesserae System",
    );
    image
}

fn draw_tile(img: &mut RgbImage, fonts: &Fonts, x: u32, y: u32, tile: &Tile, text: &TileText) {
    let (bg, fg) = if tile.invert {
        (BLACK, WHITE)
    } else {
        (WHITE, BLACK)
    };

    // Fill, plus a 2 px border on white tiles.
    draw_filled_rect_mut(
        img,
        Rect::at(x as i32, y as i32).of_size(TILE_W, TILE_H),
        bg,
    );
    if !tile.invert {
        draw_hollow_rect_mut(
            img,
            Rect::at(x as i32, y as i32).of_size(TILE_W, TILE_H),
            BLACK,
        );
        draw_hollow_rect_mut(
            img,
            Rect::at(x as i32 + 1, y as i32 + 1).of_size(TILE_W - 2, TILE_H - 2),
            BLACK,
        );
    }

    let tx = (x + TEXT_X) as i32;
    let y = y as i32;
    draw_text_mut(
        img,
        fg,
        tx,
        y + 30,
        Scale::uniform(BIG),
        &fonts.bold,
        &text.big,
    );
    draw_text_mut(
        img,
        fg,
        tx + SUB_INDENT as i32,
        y + 130,
        Scale::uniform(SUB),
        &fonts.light,
        &text.sub,
    );
    for (i, line) in text.lines.iter().enumerate() {
        draw_text_mut(
            img,
            fg,
            tx,
            y + 190 + 42 * i as i32,
            Scale::uniform(LINE),
            &fonts.bold,
            line,
        );
    }
    draw_text_mut(
        img,
        fg,
        tx,
        y + TILE_H as i32 - 55,
        Scale::uniform(NAME),
        &fonts.bold,
        tile.name,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, Utc};
    use rusttype::point;

    fn text_width(font: &Font, size: f32, text: &str) -> u32 {
        font.layout(text, Scale::uniform(size), point(0.0, 0.0))
            .filter_map(|g| g.pixel_bounding_box())
            .map(|b| b.max.x.max(0) as u32)
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn tile_text_for_windows_and_problems() {
        let now = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2026, 10, 12, 12, 0, 0)
            .unwrap();
        let windows = vec![
            Window {
                label: "5H",
                used_percent: 27.4,
                resets_at: Some(Utc.with_ymd_and_hms(2026, 10, 12, 17, 0, 0).unwrap()),
            },
            Window {
                label: "WK",
                used_percent: 80.0,
                resets_at: None,
            },
        ];
        assert_eq!(
            tile_text(&now, &Ok(windows)),
            TileText {
                big: "20%".to_string(),
                sub: "left · WK".to_string(),
                lines: vec!["5H 73% · 17:00".to_string(), "WK 20% · --".to_string()],
            }
        );
        assert_eq!(tile_text(&now, &Ok(vec![])).sub, "no data");
        assert_eq!(
            tile_text(&now, &Err(Problem::SignInExpired)).sub,
            "sign-in expired"
        );
    }

    #[test]
    fn worst_case_text_fits_in_a_tile() {
        let fonts = Fonts::load();
        let room = TILE_W - 2 * TEXT_X;
        assert!(text_width(&fonts.bold, BIG, "100%") <= room);
        assert!(text_width(&fonts.bold, NAME, "CLAUDE") <= room);
        let problems = [
            Problem::NotSetUp,
            Problem::SignInExpired,
            Problem::NoAccess,
            Problem::RateLimited,
            Problem::Http(503),
            Problem::Offline,
            Problem::BadData,
        ];
        let subs = problems
            .iter()
            .map(Problem::text)
            .chain(["no data".to_string(), "left · DAY".to_string()]);
        for sub in subs {
            assert!(
                text_width(&fonts.light, SUB, &sub) <= room - SUB_INDENT,
                "too wide: {sub}"
            );
        }
        for day in ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"] {
            let line = format!("DAY 100% · {day} 23:59");
            assert!(
                text_width(&fonts.bold, LINE, &line) <= room,
                "too wide: {line}"
            );
        }
    }

    #[test]
    fn renders_a_full_size_dashboard() {
        let tiles = [
            Tile {
                name: "CLAUDE",
                quota: Ok(vec![Window {
                    label: "5H",
                    used_percent: 10.0,
                    resets_at: None,
                }]),
                invert: true,
            },
            Tile {
                name: "CODEX",
                quota: Ok(vec![]),
                invert: false,
            },
            Tile {
                name: "Z.AI",
                quota: Err(Problem::Offline),
                invert: false,
            },
            Tile {
                name: "GROK",
                quota: Err(Problem::SignInExpired),
                invert: true,
            },
        ];
        let image = render(&Local::now(), &tiles, &Fonts::load());
        assert_eq!(image.dimensions(), (WIDTH, HEIGHT));
    }
}

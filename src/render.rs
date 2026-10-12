use crate::quota::{format_reset, percent_left, tightest, Problem, Window};
use chrono::{DateTime, Local, TimeZone};
use image::imageops::rotate90;
use image::{DynamicImage, GrayImage, Rgb, RgbImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_hollow_rect_mut, draw_text_mut};
use imageproc::rect::Rect;
use rusttype::{point, Font, Scale};
use std::fmt::Display;

/// The Kindle framebuffer, portrait.
pub const WIDTH: u32 = 758;
pub const HEIGHT: u32 = 1024;
/// The dashboard is drawn landscape, for a Kindle standing on its long side with its top edge
/// to the left, then turned 90 degrees clockwise into the portrait framebuffer.
const CANVAS_W: u32 = HEIGHT;
const CANVAS_H: u32 = WIDTH;

const PADDING: u32 = 15;
const GRID_TOP: u32 = 120;
const TILE_W: u32 = (CANVAS_W - PADDING * 3) / 2;
const TILE_H: u32 = (CANVAS_H - GRID_TOP - PADDING * 2) / 2;
/// Left margin of the text inside a tile.
const TEXT_X: u32 = 25;
const SUB_INDENT: u32 = 5;
/// The window lines sit in a right column; the big number and its sub line must stay left of it.
const RIGHT_X: u32 = 290;

const BIG: f32 = 96.0;
const SUB: f32 = 30.0;
const WINDOW: f32 = 34.0;
const RESET: f32 = 28.0;
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
    /// Per window: "<label> <left>%" and its reset time.
    pub windows: Vec<(String, String)>,
}

pub fn tile_text<Tz: TimeZone>(now: &DateTime<Tz>, quota: &Result<Vec<Window>, Problem>) -> TileText
where
    Tz::Offset: Display,
{
    let empty = |sub: String| TileText {
        big: "--".to_string(),
        sub,
        windows: Vec::new(),
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
        windows: windows
            .iter()
            .map(|w| {
                (
                    format!("{} {}%", w.label, percent_left(w)),
                    format_reset(now, w.resets_at),
                )
            })
            .collect(),
    }
}

/// The Kindle's `eips -g` needs an 8-bit grayscale PNG: it draws an RGB one stretched to twice
/// the width, so only the left half of the dashboard shows.
pub fn render(now: &DateTime<Local>, tiles: &[Tile], fonts: &Fonts) -> GrayImage {
    let mut canvas = RgbImage::from_pixel(CANVAS_W, CANVAS_H, WHITE);

    // Clock and date across the top: the time of this render.
    let time_str = now.format("%H:%M").to_string();
    let date_str = now.format("%A, %d %b").to_string();
    draw_text_mut(
        &mut canvas,
        BLACK,
        PADDING as i32,
        10,
        Scale::uniform(100.0),
        &fonts.bold,
        &time_str,
    );
    draw_text_mut(
        &mut canvas,
        BLACK,
        PADDING as i32 + 290,
        50,
        Scale::uniform(45.0),
        &fonts.light,
        &date_str,
    );
    let footer = "Headroom Tesserae System";
    let footer_w = text_width(&fonts.light, 22.0, footer);
    draw_text_mut(
        &mut canvas,
        BLACK,
        (CANVAS_W - PADDING - footer_w) as i32,
        68,
        Scale::uniform(22.0),
        &fonts.light,
        footer,
    );

    // 2x2 grid, filled row by row.
    for (i, tile) in tiles.iter().enumerate() {
        let x = PADDING + (i as u32 % 2) * (TILE_W + PADDING);
        let y = GRID_TOP + (i as u32 / 2) * (TILE_H + PADDING);
        draw_tile(&mut canvas, fonts, x, y, tile, &tile_text(now, &tile.quota));
    }

    DynamicImage::ImageRgb8(rotate90(&canvas)).into_luma8()
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

    // Left column: big number, its window, and the provider name at the bottom.
    let tx = (x + TEXT_X) as i32;
    let y = y as i32;
    draw_text_mut(
        img,
        fg,
        tx,
        y + 25,
        Scale::uniform(BIG),
        &fonts.bold,
        &text.big,
    );
    draw_text_mut(
        img,
        fg,
        tx + SUB_INDENT as i32,
        y + 135,
        Scale::uniform(SUB),
        &fonts.light,
        &text.sub,
    );
    draw_text_mut(
        img,
        fg,
        tx,
        y + TILE_H as i32 - 55,
        Scale::uniform(NAME),
        &fonts.bold,
        tile.name,
    );

    // Right column: two lines per window, percent left then reset time.
    let rx = (x + RIGHT_X) as i32;
    for (i, (head, reset)) in text.windows.iter().enumerate() {
        let wy = y + 35 + 100 * i as i32;
        draw_text_mut(img, fg, rx, wy, Scale::uniform(WINDOW), &fonts.bold, head);
        draw_text_mut(
            img,
            fg,
            rx,
            wy + 42,
            Scale::uniform(RESET),
            &fonts.light,
            reset,
        );
    }
}

/// Width in pixels of `text` drawn at `size`.
fn text_width(font: &Font, size: f32, text: &str) -> u32 {
    font.layout(text, Scale::uniform(size), point(0.0, 0.0))
        .filter_map(|g| g.pixel_bounding_box())
        .map(|b| b.max.x.max(0) as u32)
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, Utc};

    const LEFT_ROOM: u32 = RIGHT_X - TEXT_X - 10;
    const RIGHT_ROOM: u32 = TILE_W - RIGHT_X - 20;

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
                windows: vec![
                    ("5H 73%".to_string(), "17:00".to_string()),
                    ("WK 20%".to_string(), "--".to_string()),
                ],
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
        assert!(text_width(&fonts.bold, BIG, "100%") <= LEFT_ROOM);
        assert!(text_width(&fonts.bold, NAME, "CLAUDE") <= LEFT_ROOM);
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
                text_width(&fonts.light, SUB, &sub) <= LEFT_ROOM - SUB_INDENT,
                "too wide: {sub}"
            );
        }
        assert!(text_width(&fonts.bold, WINDOW, "DAY 100%") <= RIGHT_ROOM);
        for day in ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"] {
            let reset = format!("{day} 23:59");
            assert!(
                text_width(&fonts.light, RESET, &reset) <= RIGHT_ROOM,
                "too wide: {reset}"
            );
        }
    }

    #[test]
    fn renders_a_portrait_framebuffer_from_a_landscape_canvas() {
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
        // The first tile (black, top-left of the landscape canvas) lands at the top-right of
        // the framebuffer after the clockwise turn.
        let (cx, cy) = (PADDING + 5, GRID_TOP + 5);
        assert_eq!(image.get_pixel(WIDTH - 1 - cy, cx).0, [0]);
        assert_eq!(image.get_pixel(5, 5).0, [255]);
    }
}

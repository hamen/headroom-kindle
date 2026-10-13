use crate::quota::{format_reset, percent_left, tightest, Problem, Window};
use chrono::{DateTime, Local, TimeZone};
use image::imageops::rotate270;
use image::{DynamicImage, GrayImage, ImageOutputFormat, Rgb, RgbImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_hollow_rect_mut, draw_text_mut};
use imageproc::rect::Rect;
use rusttype::{point, Font, Scale};
use std::fmt::Display;
use std::io::Cursor;

/// The Kindle framebuffer, portrait.
pub const WIDTH: u32 = 758;
pub const HEIGHT: u32 = 1024;
/// The dashboard is drawn landscape, for a Kindle standing on its long side with its top edge
/// to the right (USB port and charger on the left), then turned 90 degrees counterclockwise into
/// the portrait framebuffer.
const CANVAS_W: u32 = HEIGHT;
const CANVAS_H: u32 = WIDTH;

const PADDING: u32 = 15;
/// A thin header row: the date, and when this image was made.
const GRID_TOP: u32 = 60;
const HEADER: f32 = 32.0;
const TILE_W: u32 = (CANVAS_W - PADDING * 3) / 2;
const TILE_H: u32 = (CANVAS_H - GRID_TOP - PADDING * 2) / 2;
/// Left margin of the text inside a tile.
const TEXT_X: u32 = 25;
const SUB_INDENT: u32 = 5;
/// The window lines sit in a right column; the big number and its sub line must stay left of it.
const RIGHT_X: u32 = 265;

/// All tile text is bold: thin text washes out on this e-ink, worst of all white on black.
const BIG: f32 = 88.0;
const SUB: f32 = 34.0;
const WINDOW: f32 = 40.0;
const RESET: f32 = 36.0;
const NAME: f32 = 44.0;

/// A tile turns black when its tightest window has less than this percent left.
const LOW_LEFT: i32 = 20;

const BLACK: Rgb<u8> = Rgb([0, 0, 0]);
const WHITE: Rgb<u8> = Rgb([255, 255, 255]);

pub struct Fonts {
    bold: Font<'static>,
}

impl Fonts {
    pub fn load() -> Fonts {
        // Embedded, so the binary runs from any directory (the systemd timer).
        Fonts {
            bold: Font::try_from_bytes(include_bytes!("../assets/font.ttf")).unwrap(),
        }
    }
}

/// One provider tile: its name, and its windows or the reason it has none.
pub struct Tile {
    pub name: &'static str,
    pub quota: Result<Vec<Window>, Problem>,
}

/// Tiles are white; a tile with little quota left turns black, so it stands out.
fn is_low(quota: &Result<Vec<Window>, Problem>) -> bool {
    match quota {
        Ok(windows) => tightest(windows).is_some_and(|w| percent_left(w) < LOW_LEFT),
        Err(_) => false,
    }
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

    // The screen changes only every 10 minutes, so no clock: the date, and when the numbers
    // were fetched. When the PC is off the image stays, and this time shows how old it is.
    let date_str = now.format("%A, %d %b").to_string();
    let updated = format!("updated {}", now.format("%H:%M"));
    let updated_x = CANVAS_W - PADDING - text_width(&fonts.bold, HEADER, &updated);
    draw_text_mut(
        &mut canvas,
        BLACK,
        PADDING as i32,
        14,
        Scale::uniform(HEADER),
        &fonts.bold,
        &date_str,
    );
    draw_text_mut(
        &mut canvas,
        BLACK,
        updated_x as i32,
        14,
        Scale::uniform(HEADER),
        &fonts.bold,
        &updated,
    );

    // 2x2 grid, filled row by row.
    for (i, tile) in tiles.iter().enumerate() {
        let x = PADDING + (i as u32 % 2) * (TILE_W + PADDING);
        let y = GRID_TOP + (i as u32 / 2) * (TILE_H + PADDING);
        draw_tile(&mut canvas, fonts, x, y, tile, &tile_text(now, &tile.quota));
    }

    DynamicImage::ImageRgb8(rotate270(&canvas)).into_luma8()
}

/// PNG bytes of the dashboard, ready to save or to send to the Kindle.
pub fn png_bytes(image: &GrayImage) -> Vec<u8> {
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageOutputFormat::Png)
        .expect("PNG encoding to memory");
    png
}

fn draw_tile(img: &mut RgbImage, fonts: &Fonts, x: u32, y: u32, tile: &Tile, text: &TileText) {
    let low = is_low(&tile.quota);
    let (bg, fg) = if low { (BLACK, WHITE) } else { (WHITE, BLACK) };

    // Fill, plus a 2 px border on white tiles.
    draw_filled_rect_mut(
        img,
        Rect::at(x as i32, y as i32).of_size(TILE_W, TILE_H),
        bg,
    );
    if !low {
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
        &fonts.bold,
        &text.sub,
    );
    draw_text_mut(
        img,
        fg,
        tx,
        y + TILE_H as i32 - 62,
        Scale::uniform(NAME),
        &fonts.bold,
        tile.name,
    );

    // Right column: two lines per window, percent left then reset time.
    let rx = (x + RIGHT_X) as i32;
    for (i, (head, reset)) in text.windows.iter().enumerate() {
        let wy = y + 35 + 125 * i as i32;
        draw_text_mut(img, fg, rx, wy, Scale::uniform(WINDOW), &fonts.bold, head);
        draw_text_mut(
            img,
            fg,
            rx,
            wy + 50,
            Scale::uniform(RESET),
            &fonts.bold,
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
    /// A tile with an error has no window column, so its text can use the whole width.
    const FULL_ROOM: u32 = TILE_W - 2 * TEXT_X;

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

    fn used(used_percent: f64) -> Result<Vec<Window>, Problem> {
        Ok(vec![Window {
            label: "WK",
            used_percent,
            resets_at: None,
        }])
    }

    #[test]
    fn only_a_tile_with_little_left_is_black() {
        assert!(is_low(&used(81.0)));
        assert!(!is_low(&used(80.0)));
        assert!(!is_low(&Ok(vec![])));
        assert!(!is_low(&Err(Problem::Offline)));
    }

    #[test]
    fn worst_case_text_fits_in_a_tile() {
        let fonts = Fonts::load();
        assert!(text_width(&fonts.bold, BIG, "100%") <= LEFT_ROOM);
        assert!(text_width(&fonts.bold, NAME, "CLAUDE") <= LEFT_ROOM);
        let header = text_width(&fonts.bold, HEADER, "Wednesday, 30 Sep")
            + text_width(&fonts.bold, HEADER, "updated 23:59");
        assert!(header + 3 * PADDING <= CANVAS_W);
        let problems = [
            Problem::NotSetUp,
            Problem::SignInExpired,
            Problem::NoAccess,
            Problem::RateLimited,
            Problem::Http(503),
            Problem::Offline,
            Problem::BadData,
        ];
        for sub in problems
            .iter()
            .map(Problem::text)
            .chain(["no data".to_string()])
        {
            assert!(
                text_width(&fonts.bold, SUB, &sub) <= FULL_ROOM - SUB_INDENT,
                "too wide: {sub}"
            );
        }
        assert!(text_width(&fonts.bold, SUB, "left · DAY") <= LEFT_ROOM - SUB_INDENT);
        assert!(text_width(&fonts.bold, WINDOW, "DAY 100%") <= RIGHT_ROOM);
        for day in ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"] {
            let reset = format!("{day} 23:59");
            assert!(
                text_width(&fonts.bold, RESET, &reset) <= RIGHT_ROOM,
                "too wide: {reset}"
            );
        }
    }

    #[test]
    fn renders_a_portrait_framebuffer_from_a_landscape_canvas() {
        let tiles = [
            Tile {
                name: "CLAUDE",
                quota: used(90.0),
            },
            Tile {
                name: "CODEX",
                quota: used(10.0),
            },
            Tile {
                name: "Z.AI",
                quota: Err(Problem::Offline),
            },
            Tile {
                name: "GROK",
                quota: Ok(vec![]),
            },
        ];
        let image = render(&Local::now(), &tiles, &Fonts::load());
        assert_eq!(image.dimensions(), (WIDTH, HEIGHT));
        // eips needs an 8-bit grayscale PNG; an RGB one is drawn stretched.
        let decoded = image::load_from_memory(&png_bytes(&image)).unwrap();
        assert_eq!(decoded.color(), image::ColorType::L8);
        assert_eq!((decoded.width(), decoded.height()), (WIDTH, HEIGHT));
        // A landscape canvas point (cx, cy) lands at (cy, HEIGHT - 1 - cx) after the
        // counterclockwise turn.
        let at = |cx: u32, cy: u32| image.get_pixel(cy, HEIGHT - 1 - cx).0;
        // Claude has 10% left: black tile. Codex has 90% left: white tile inside its border.
        assert_eq!(at(PADDING + 5, GRID_TOP + 5), [0]);
        assert_eq!(at(PADDING * 2 + TILE_W + 5, GRID_TOP + 5), [255]);
        // The bottom-left corner of the canvas is margin.
        assert_eq!(at(5, CANVAS_H - 6), [255]);
    }
}

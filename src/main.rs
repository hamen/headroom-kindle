use image::{Rgb, RgbImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_text_mut, draw_hollow_rect_mut};
use imageproc::rect::Rect;
use rusttype::{Font, Scale};
use std::fs;
use chrono::Local;

fn main() {
    let width = 758;
    let height = 1024;
    
    // Tesserae/Hass-Eink style: BIANCO, PULITO, TILES (Piastrelle)
    let mut image = RgbImage::from_pixel(width, height, Rgb([255, 255, 255]));

    let font_bold_data = fs::read("assets/font.ttf").expect("Errore font.ttf");
    let font_bold = Font::try_from_vec(font_bold_data).unwrap();

    let font_light_data = fs::read("assets/font_light.ttf").expect("Errore font_light.ttf");
    let font_light = Font::try_from_vec(font_light_data).unwrap();

    let black = Rgb([0, 0, 0]);

    // GRID SETTINGS
    let cols = 2;
    let padding = 15;
    let tile_w = (width - padding * (cols + 1)) / cols;
    let tile_h = 280; // Piastrelle quadrate/rettangolari in stile Windows Phone / Tesserae

    // DATA/ORA GIGANTE IN CIMA (Tipico dei display da muro)
    let now = Local::now();
    let time_str = now.format("%H:%M").to_string();
    let date_str = now.format("%A, %d %b").to_string();
    
    draw_text_mut(&mut image, black, padding as i32, 20, Scale{x: 100.0, y: 100.0}, &font_bold, &time_str);
    draw_text_mut(&mut image, black, padding as i32 + 300, 70, Scale{x: 45.0, y: 45.0}, &font_light, &date_str);
    
    let mut current_y = 150;

    // Helper per TESSERAE (Tile Nera con Testo Bianco, o viceversa)
    let draw_tile = |img: &mut RgbImage, x: u32, y: u32, w: u32, h: u32, title: &str, val: &str, unit: &str, invert: bool| {
        let (bg, fg) = if invert { (black, Rgb([255, 255, 255])) } else { (Rgb([255, 255, 255]), black) };
        
        // Fill base + bordo (se bianco)
        draw_filled_rect_mut(img, Rect::at(x as i32, y as i32).of_size(w, h), bg);
        if !invert {
            draw_hollow_rect_mut(img, Rect::at(x as i32, y as i32).of_size(w, h), black);
            draw_hollow_rect_mut(img, Rect::at(x as i32+1, y as i32+1).of_size(w-2, h-2), black); // Bordo spesso 2px
        }

        // Valore Gigante
        draw_text_mut(img, fg, x as i32 + 25, y as i32 + 50, Scale{x: 90.0, y: 90.0}, &font_bold, val);
        
        // Unità (Sotto il valore)
        draw_text_mut(img, fg, x as i32 + 30, y as i32 + 160, Scale{x: 35.0, y: 35.0}, &font_light, unit);
        
        // Titolo (In basso)
        draw_text_mut(img, fg, x as i32 + 25, y as i32 + h as i32 - 50, Scale{x: 35.0, y: 35.0}, &font_bold, title);
    };

    // RIGA 1: CLAUDE e Z.AI
    draw_tile(&mut image, padding, current_y, tile_w, tile_h, "CLAUDE PRO", "15", "messages left", true); // Tile Nera
    draw_tile(&mut image, padding * 2 + tile_w, current_y, tile_w, tile_h, "Z.AI TOKENS", "1.2", "million used", false); // Tile Bianca
    
    current_y += tile_h + padding;

    // RIGA 2: GITHUB COPILOT (Tile Orizzontale Larga)
    draw_tile(&mut image, padding, current_y, width - (padding * 2), 200, "GITHUB COPILOT", "OK", "Active (GPT-4o)", true);

    current_y += 200 + padding;

    // RIGA 3: SYSTEM INFO E METEO
    draw_tile(&mut image, padding, current_y, tile_w, tile_h, "MONEYMAKER", "22%", "CPU Load", false);
    draw_tile(&mut image, padding * 2 + tile_w, current_y, tile_w, tile_h, "ROOM TEMP", "24°", "Humidity 40%", false);

    // Footer
    draw_text_mut(&mut image, black, padding as i32, height as i32 - 40, Scale{x: 25.0, y: 25.0}, &font_light, "Headroom Tesserae System");

    image.save("dashboard.png").unwrap();
}

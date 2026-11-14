//! Texto dentro de imágenes (póster, GIF vertical), con la fuente de mapa de bits 8×8 de
//! `font8x8`: ASCII y Latin-1, así que cubre tildes, ñ, ¿ y ¡.

use font8x8::{BASIC_FONTS, LATIN_FONTS, UnicodeFonts};
use image::RgbImage;

use crate::color::Rgb;

fn glyph(c: char) -> [u8; 8] {
    BASIC_FONTS.get(c).or_else(|| LATIN_FONTS.get(c)).or_else(|| BASIC_FONTS.get(fallback(c))).unwrap_or([0; 8])
}

/// Caracteres comunes fuera de Latin-1, cambiados por uno parecido.
fn fallback(c: char) -> char {
    match c {
        '·' | '•' => '.',
        '—' | '–' => '-',
        '“' | '”' => '"',
        '‘' | '’' => '\'',
        '…' => '.',
        _ => '?',
    }
}

/// Ancho en píxeles de `text` con escala `scale`.
pub fn width(text: &str, scale: u32) -> u32 {
    text.chars().count() as u32 * 8 * scale
}

/// Recorta `text` para que quepa en `max` píxeles, con "..." si se corta.
pub fn fit(text: &str, max: u32, scale: u32) -> String {
    if width(text, scale) <= max {
        return text.to_string();
    }
    let room = (max / (8 * scale)).saturating_sub(3) as usize;
    format!("{}...", text.chars().take(room).collect::<String>().trim_end())
}

/// Escribe `text` con su esquina superior izquierda en (`x`, `y`).
pub fn draw(img: &mut RgbImage, x: i64, y: i64, text: &str, color: Rgb, scale: u32) {
    let (r, g, b) = color.to_u8();
    for (n, c) in text.chars().enumerate() {
        let rows = glyph(c);
        let x0 = x + (n as u32 * 8 * scale) as i64;
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..8 {
                if bits >> col & 1 == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let px = x0 + (col * scale + dx) as i64;
                        let py = y + (row as u32 * scale + dy) as i64;
                        if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                            img.put_pixel(px as u32, py as u32, image::Rgb([r, g, b]));
                        }
                    }
                }
            }
        }
    }
}

/// Escribe `text` centrado en la columna `cx`.
pub fn draw_centered(img: &mut RgbImage, cx: i64, y: i64, text: &str, color: Rgb, scale: u32) {
    draw(img, cx - width(text, scale) as i64 / 2, y, text, color, scale);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_spanish_text_and_trims_it() {
        let mut img = RgbImage::new(200, 20);
        draw(&mut img, 0, 0, "Canción ñandú ¿sí?", Rgb::WHITE, 1);
        let lit = img.pixels().filter(|p| p[0] > 0).count();
        assert!(lit > 100, "casi no dibujó: {lit}");
        // La tilde dibuja algo distinto de la letra sin tilde.
        assert_ne!(glyph('ó'), glyph('o'));
        assert_ne!(glyph('ñ'), [0; 8]);
        assert_eq!(fit("una canción muy larga", 80, 1), "una ca...");
        assert_eq!(fit("corta", 80, 1), "corta");
        // Fuera de la imagen no entra en pánico.
        draw(&mut img, -50, -50, "fuera", Rgb::WHITE, 3);
        draw(&mut img, 190, 15, "borde", Rgb::WHITE, 2);
    }
}

//! Modo karaoke (tecla `k`): la línea que suena en letras grandes, al centro de la escena.
//!
//! Cada letra es un carácter de la fuente 8×8 dibujado con píxeles del lienzo, así que
//! ocupa 8 columnas y 4 filas de la terminal. La línea se corta en renglones por palabras;
//! si no cabe en tres, se usa una letra de la mitad del tamaño. Se ilumina de izquierda a
//! derecha a medida que se canta.

use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::text::glyph;

/// Corta `text` en renglones de a lo más `max` caracteres, sin partir palabras (salvo
/// una palabra más larga que el renglón).
pub fn wrap(text: &str, max: usize) -> Vec<String> {
    let max = max.max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let mut word: String = word.to_string();
        while word.chars().count() > max {
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            let head: String = word.chars().take(max).collect();
            word = word.chars().skip(max).collect();
            lines.push(head);
        }
        let need = line.chars().count() + usize::from(!line.is_empty()) + word.chars().count();
        if need > max && !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Dibuja la línea en grande. Devuelve la fila (en celdas) justo debajo del texto, para
/// poner ahí la línea siguiente, o `None` si no cupo.
pub fn draw(canvas: &mut Canvas, text: &str, progress: f32, pal: &Palette) -> Option<u16> {
    let (w, h) = (canvas.w, canvas.px_h());
    if text.trim().is_empty() || w < 16 || h < 16 {
        return None;
    }
    // Primero a tamaño completo (8 px); si no cabe en tres renglones, a la mitad (4 px).
    let (size, lines) = [8usize, 4]
        .into_iter()
        .map(|size| (size, wrap(text, (w.saturating_sub(4)) / size)))
        .find(|(size, lines)| lines.len() <= 3 && lines.len() * (size + size / 2) <= h / 2)
        .unwrap_or_else(|| (4, wrap(text, (w.saturating_sub(4)) / 4).into_iter().take(3).collect()));
    let line_h = size + size / 2;
    let block = lines.len() * line_h;
    let top = (h.saturating_sub(block)) / 2;
    // Fondo más oscuro detrás del texto, para leerlo sobre cualquier escena.
    canvas.shade(top.saturating_sub(4), (top + block + 4).min(h), 0.65);
    let total: usize = lines.iter().map(|l| l.chars().count()).sum::<usize>().max(1);
    let lit = (progress.clamp(0.0, 1.0) * total as f32).ceil() as usize;
    let sung = pal.petal_light.mix(Rgb::WHITE, 0.7);
    let waiting = Rgb::WHITE.scale(0.55);
    let mut seen = 0usize;
    for (row, line) in lines.iter().enumerate() {
        let n = line.chars().count();
        let x0 = (w.saturating_sub(n * size)) / 2;
        let y0 = top + row * line_h;
        for (i, c) in line.chars().enumerate() {
            let color = if seen + i < lit { sung } else { waiting };
            let rows = glyph(c);
            for gy in 0..size {
                for gx in 0..size {
                    // A tamaño medio, un píxel vale por un cuadro de 2×2 de la letra.
                    let on = if size == 8 {
                        rows[gy] >> gx & 1 == 1
                    } else {
                        (0..2).any(|dy| (0..2).any(|dx| rows[gy * 2 + dy] >> (gx * 2 + dx) & 1 == 1))
                    };
                    if on {
                        canvas.solid((x0 + i * size + gx) as i32, (y0 + gy) as i32, color);
                    }
                }
            }
        }
        seen += n;
    }
    Some(((top + block) / 2 + 1) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_by_words() {
        assert_eq!(wrap("me quedo mirando el cielo", 10), vec!["me quedo", "mirando el", "cielo"]);
        assert_eq!(wrap("supercalifragilístico", 8), vec!["supercal", "ifragilí", "stico"]);
        assert_eq!(wrap("  ", 5), Vec::<String>::new());
        assert_eq!(wrap("hola", 0), vec!["h", "o", "l", "a"]);
    }

    #[test]
    fn draws_big_and_lights_up_as_sung() {
        let pal = Palette::default();
        let count = |progress: f32| {
            let mut c = Canvas::new(120, 30);
            c.clear(|_, _| Rgb::BLACK);
            let below = draw(&mut c, "hola mundo", progress, &pal).unwrap();
            assert!(below > 0 && (below as usize) < 30);
            let img = c.to_image();
            let sung = pal.petal_light.mix(Rgb::WHITE, 0.7).to_u8();
            img.pixels().filter(|p| (p[0], p[1], p[2]) == sung).count()
        };
        assert_eq!(count(0.0), 0);
        let half = count(0.5);
        let all = count(1.0);
        assert!(half > 0 && all > half, "{half} {all}");
    }

    #[test]
    fn long_lines_shrink_and_tiny_canvases_skip() {
        let pal = Palette::default();
        let mut c = Canvas::new(60, 20);
        assert!(draw(&mut c, &"palabra ".repeat(12), 0.5, &pal).is_some());
        let mut tiny = Canvas::new(10, 5);
        assert!(draw(&mut tiny, "hola", 0.5, &pal).is_none());
    }
}

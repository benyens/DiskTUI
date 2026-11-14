//! Póster del herbario y resumen de la semana.
//!
//! El póster pone todas las flores en una grilla, con título, artista y fecha bajo cada
//! una, y lo guarda como PNG en `~/Pictures/DiskTUI`.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};

use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::herbarium::{self, Entry};
use crate::mood::Mood;
use crate::palette::Palette;
use crate::postcard;
use crate::text;

/// Ficha del póster, en celdas del lienzo (cada celda mide 8×16 píxeles en la imagen).
const TILE: (usize, usize) = (26, 12);
const HEADER: usize = 4;
const FOOTER: usize = 2;
const MAX_COLUMNS: usize = 5;

/// Día número `n` desde 1970-01-01 para una fecha `AAAA-MM-DD`.
fn day_number(date: &str) -> Option<i64> {
    let mut parts = date.trim().splitn(3, '-').map(|p| p.parse::<i64>().ok());
    let (y, m, d) = (parts.next()??, parts.next()??, parts.next()??);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Algoritmo de días civiles (Howard Hinnant).
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// Lo escuchado en los últimos siete días.
#[derive(Debug, PartialEq)]
pub struct Week {
    pub flowers: usize,
    pub mood: Option<Mood>,
    pub bpm: Option<f32>,
}

impl Week {
    pub fn of(entries: &[Entry], today: &str) -> Week {
        let Some(now) = day_number(today) else { return Week { flowers: 0, mood: None, bpm: None } };
        let recent: Vec<&Entry> =
            entries.iter().filter(|e| day_number(&e.date).is_some_and(|d| (0..7).contains(&(now - d)))).collect();
        let mood = Mood::ALL
            .into_iter()
            .map(|m| (m, recent.iter().filter(|e| Mood::parse(&e.mood) == Some(m)).count()))
            .filter(|(_, n)| *n > 0)
            .max_by_key(|(_, n)| *n)
            .map(|(m, _)| m);
        let tempos: Vec<f32> = recent.iter().map(|e| e.bpm).filter(|b| *b > 0.0).collect();
        let bpm = (!tempos.is_empty()).then(|| tempos.iter().sum::<f32>() / tempos.len() as f32);
        Week { flowers: recent.len(), mood, bpm }
    }

    /// Una línea para el herbario: "esta semana: 5 flores · mayormente nostálgica · 104 BPM".
    pub fn line(&self) -> Option<String> {
        if self.flowers == 0 {
            return None;
        }
        let mut text = format!("esta semana: {} {}", self.flowers, if self.flowers == 1 { "flor" } else { "flores" });
        if let Some(m) = self.mood {
            text.push_str(&format!(" · mayormente {}", m.name()));
        }
        if let Some(b) = self.bpm {
            text.push_str(&format!(" · {b:.0} BPM en promedio"));
        }
        Some(text)
    }
}

/// Arma la imagen del póster. Lo más nuevo primero.
pub fn render(entries: &[Entry], week: &Week) -> image::RgbImage {
    let n = entries.len().max(1);
    let cols = ((n as f32).sqrt().ceil() as usize).clamp(1, MAX_COLUMNS).min(n);
    let rows = n.div_ceil(cols);
    let (w, h) = (cols * TILE.0, HEADER + rows * TILE.1 + FOOTER);
    let pal = Palette::default();
    let mut canvas = Canvas::new(w, h);
    let ph = canvas.px_h() as f32;
    canvas.clear(|_, y| pal.bg_top.mix(pal.bg_bottom, y as f32 / ph));
    for (i, entry) in entries.iter().rev().enumerate() {
        let (col, row) = (i % cols, i / cols);
        let cx = (col * TILE.0) as f32 + TILE.0 as f32 / 2.0;
        let cy = ((HEADER + row * TILE.1) as f32 + 3.0) * 2.0;
        herbarium::draw_specimen(&mut canvas, entry, cx, cy, 5.0, i as f32 * 1.7);
    }
    let mut img = canvas.to_image();

    // Textos: 8 píxeles por celda de ancho, 16 de alto.
    let (cw, ch) = (8i64, 16i64);
    let bright = pal.bg_top.mix(Rgb::WHITE, 0.92);
    let dim = pal.bg_top.mix(Rgb::WHITE, 0.55);
    let accent = pal.petal.mix(Rgb::WHITE, 0.35);
    let center = img.width() as i64 / 2;
    let title = text::fit("Herbario", img.width().saturating_sub(16), 3);
    text::draw_centered(&mut img, center, 12, &title, accent, 3);
    let count = format!("{} {}", entries.len(), if entries.len() == 1 { "flor" } else { "flores" });
    let mut sub = count;
    if week.flowers > 0 {
        sub.push_str(&format!(" · {} esta semana", week.flowers));
        if let Some(m) = week.mood {
            sub.push_str(&format!(" · mayormente {}", m.name()));
        }
    }
    let sub = text::fit(&sub, img.width().saturating_sub(16), 1);
    text::draw_centered(&mut img, center, 44, &sub, dim, 1);
    for (i, entry) in entries.iter().rev().enumerate() {
        let (col, row) = (i % cols, i / cols);
        let cx = ((col * TILE.0) as i64 + TILE.0 as i64 / 2) * cw;
        let y0 = (HEADER + row * TILE.1) as i64 * ch + 9 * ch;
        let room = ((TILE.0 - 2) as i64 * cw) as u32;
        text::draw_centered(&mut img, cx, y0, &text::fit(&entry.title, room, 1), bright, 1);
        text::draw_centered(&mut img, cx, y0 + 14, &text::fit(&entry.artist, room, 1), dim, 1);
        text::draw_centered(&mut img, cx, y0 + 28, &text::fit(&entry.date, room, 1), dim.mix(pal.bg_top, 0.35), 1);
    }
    let footer_y = img.height() as i64 - FOOTER as i64 * ch + 8;
    text::draw_centered(&mut img, center, footer_y, "DiskTUI", dim.mix(pal.bg_top, 0.3), 1);
    // Al doble, con píxeles nítidos: se ve bien al compartirlo.
    image::imageops::resize(&img, img.width() * 2, img.height() * 2, image::imageops::FilterType::Nearest)
}

/// Guarda el póster en `~/Pictures/DiskTUI/` y devuelve la ruta.
pub fn save(entries: &[Entry], week: &Week) -> Result<PathBuf> {
    if entries.is_empty() {
        return Err(anyhow!("el herbario está vacío"));
    }
    let path = postcard::target("", "Herbario", "png")?;
    render(entries, week).save(&path).with_context(|| format!("no se pudo guardar {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(date: &str, mood: &str, bpm: f32) -> Entry {
        Entry {
            date: date.into(),
            title: format!("Canción {date}"),
            artist: "Banda".into(),
            bpm,
            key: 0,
            minor: false,
            mood: mood.into(),
            genes: crate::plant::Genes::from_song(&crate::features::Song::default()),
            petal: Rgb(0.9, 0.4, 0.6),
            accent: Rgb(1.0, 0.8, 0.3),
            stem: Rgb(0.3, 0.6, 0.3),
        }
    }

    #[test]
    fn dates_count_days_across_months_and_years() {
        assert_eq!(day_number("1970-01-01"), Some(0));
        assert_eq!(day_number("2026-03-01").unwrap() - day_number("2026-02-28").unwrap(), 1);
        assert_eq!(day_number("2024-03-01").unwrap() - day_number("2024-02-28").unwrap(), 2, "bisiesto");
        assert_eq!(day_number("2027-01-01").unwrap() - day_number("2026-12-31").unwrap(), 1);
        assert_eq!(day_number("no es fecha"), None);
        assert_eq!(day_number("2026-13-01"), None);
    }

    #[test]
    fn week_summary_counts_only_the_last_seven_days() {
        let entries = vec![
            entry("2026-11-14", "nostálgica", 100.0),
            entry("2026-11-10", "nostálgica", 110.0),
            entry("2026-11-08", "alegre", 0.0),
            entry("2026-11-01", "triste", 70.0),
        ];
        let week = Week::of(&entries, "2026-11-14");
        assert_eq!(week, Week { flowers: 3, mood: Some(Mood::Nostalgic), bpm: Some(105.0) });
        assert_eq!(week.line().unwrap(), "esta semana: 3 flores · mayormente nostálgica · 105 BPM en promedio");
        assert_eq!(Week::of(&entries, "2027-01-01").line(), None);
    }

    #[test]
    fn poster_has_a_tile_per_flower() {
        let entries: Vec<Entry> = (1..=7).map(|d| entry(&format!("2026-11-{d:02}"), "serena", 90.0)).collect();
        let img = render(&entries, &Week::of(&entries, "2026-11-07"));
        // 7 flores: 3 columnas, 3 filas.
        assert_eq!(img.width(), (3 * TILE.0 * 8 * 2) as u32);
        assert_eq!(img.height(), ((HEADER + 3 * TILE.1 + FOOTER) * 16 * 2) as u32);
        let one = render(&entries[..1], &Week::of(&entries[..1], "2026-11-01"));
        assert_eq!(one.width(), (TILE.0 * 8 * 2) as u32);
    }
}

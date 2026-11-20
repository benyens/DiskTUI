//! Resumen de lo que escuchas, a partir del herbario: tecla `r` en el herbario.
//!
//! Para la semana, el mes o desde siempre: cuántas canciones, los artistas que más
//! sonaron, cómo se repartió el ánimo, el tempo, la tonalidad más escuchada, la canción
//! más repetida y el día con más música. Se ve en la terminal y se guarda como póster.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};

use crate::color::Rgb;
use crate::features::NOTES;
use crate::herbarium::Entry;
use crate::mood::Mood;
use crate::palette::Palette;
use crate::poster::day_number;
use crate::text;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Period {
    Week,
    Month,
    All,
}

impl Period {
    pub fn name(self) -> &'static str {
        match self {
            Period::Week => "esta semana",
            Period::Month => "este mes",
            Period::All => "desde siempre",
        }
    }

    /// Días hacia atrás que cubre, contando hoy.
    fn days(self) -> Option<i64> {
        match self {
            Period::Week => Some(7),
            Period::Month => Some(30),
            Period::All => None,
        }
    }

    pub fn step(self, step: i32) -> Self {
        let all = [Period::Week, Period::Month, Period::All];
        let i = all.iter().position(|p| *p == self).unwrap_or(0) as i32;
        all[(i + step).rem_euclid(3) as usize]
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Summary {
    pub songs: usize,
    pub distinct: usize,
    pub favorites: usize,
    /// Artistas con más canciones (hasta 5).
    pub artists: Vec<(String, usize)>,
    /// Canciones por ánimo, en el orden de `Mood::ALL`.
    pub moods: Vec<(Mood, usize)>,
    pub bpm: Option<(f32, f32, f32)>,
    pub key: Option<String>,
    /// La canción que más se repitió, si se repitió.
    pub top_song: Option<(String, String, usize)>,
    pub best_day: Option<(String, usize)>,
}

/// "2026-11-14" → "sábado 14 de noviembre".
pub fn pretty_date(date: &str) -> String {
    const MONTHS: [&str; 12] = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];
    const DAYS: [&str; 7] = ["jueves", "viernes", "sábado", "domingo", "lunes", "martes", "miércoles"];
    let mut parts = date.splitn(3, '-').map(|p| p.parse::<usize>().ok());
    match (parts.next().flatten(), parts.next().flatten(), parts.next().flatten(), day_number(date)) {
        (Some(_), Some(m), Some(d), Some(n)) if (1..=12).contains(&m) => {
            format!("{} {d} de {}", DAYS[n.rem_euclid(7) as usize], MONTHS[m - 1])
        }
        _ => date.to_string(),
    }
}

fn key_name(e: &Entry) -> String {
    format!("{} {}", NOTES[e.key as usize % 12], if e.minor { "menor" } else { "mayor" })
}

/// El de más votos; a igual cantidad, el primero que apareció (para que no salte).
fn most<K: Clone + Eq + std::hash::Hash>(items: impl Iterator<Item = K>) -> Vec<(K, usize)> {
    let mut order: Vec<K> = Vec::new();
    let mut count: HashMap<K, usize> = HashMap::new();
    for k in items {
        let n = count.entry(k.clone()).or_insert(0);
        if *n == 0 {
            order.push(k);
        }
        *n += 1;
    }
    let mut all: Vec<(K, usize)> = order.into_iter().map(|k| {
        let n = count[&k];
        (k, n)
    }).collect();
    all.sort_by_key(|k| std::cmp::Reverse(k.1));
    all
}

impl Summary {
    pub fn of(entries: &[Entry], today: &str, period: Period, is_favorite: impl Fn(&Entry) -> bool) -> Summary {
        let now = day_number(today);
        let chosen: Vec<&Entry> = entries
            .iter()
            .filter(|e| match (period.days(), now, day_number(&e.date)) {
                (None, ..) => true,
                (Some(days), Some(now), Some(d)) => (0..days).contains(&(now - d)),
                _ => false,
            })
            .collect();
        if chosen.is_empty() {
            return Summary::default();
        }
        let songs = most(chosen.iter().map(|e| (e.title.clone(), e.artist.clone())));
        let artists = most(chosen.iter().filter(|e| !e.artist.trim().is_empty()).map(|e| e.artist.clone()));
        let moods = Mood::ALL.into_iter().map(|m| (m, chosen.iter().filter(|e| Mood::parse(&e.mood) == Some(m)).count())).collect();
        let tempos: Vec<f32> = chosen.iter().map(|e| e.bpm).filter(|b| *b > 0.0).collect();
        let bpm = (!tempos.is_empty()).then(|| {
            let avg = tempos.iter().sum::<f32>() / tempos.len() as f32;
            let lo = tempos.iter().copied().fold(f32::MAX, f32::min);
            let hi = tempos.iter().copied().fold(0.0, f32::max);
            (avg, lo, hi)
        });
        let key = most(chosen.iter().filter(|e| e.bpm > 0.0).map(|e| key_name(e))).into_iter().next().map(|(k, _)| k);
        let top_song = songs.first().filter(|(_, n)| *n > 1).map(|((t, a), n)| (t.clone(), a.clone(), *n));
        let best_day = most(chosen.iter().map(|e| e.date.clone())).into_iter().next();
        Summary {
            songs: chosen.len(),
            distinct: songs.len(),
            favorites: songs.iter().filter(|((t, a), _)| chosen.iter().any(|e| &e.title == t && &e.artist == a && is_favorite(e))).count(),
            artists: artists.into_iter().take(5).collect(),
            moods,
            bpm,
            key,
            top_song,
            best_day,
        }
    }

    /// El resumen como filas de texto y barras, para la terminal o el póster.
    pub fn rows(&self, period: Period, pal: &Palette) -> Vec<Vec<Seg>> {
        let bright = pal.bg_top.mix(Rgb::WHITE, 0.92);
        let dim = pal.bg_top.mix(Rgb::WHITE, 0.55);
        let accent = pal.petal.mix(Rgb::WHITE, 0.35);
        let t = |s: &str, c: Rgb| Seg::Text(s.to_string(), c);
        let mut rows = vec![vec![t("Tu música · ", accent), t(period.name(), bright)], vec![]];
        if self.songs == 0 {
            rows.push(vec![t("Todavía no hay canciones en este período.", dim)]);
            rows.push(vec![t("Escucha una canción casi entera y queda en el herbario.", dim)]);
            return rows;
        }
        let plural = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        let mut head = vec![t(&plural(self.songs, "canción", "canciones"), bright), t(&format!(" · {}", plural(self.distinct, "distinta", "distintas")), dim)];
        if self.favorites > 0 {
            head.push(t(&format!(" · ★ {}", plural(self.favorites, "favorita", "favoritas")), accent));
        }
        rows.push(head);
        rows.push(vec![]);
        if !self.artists.is_empty() {
            rows.push(vec![t("Artistas", accent)]);
            let top = self.artists[0].1.max(1);
            let width = self.artists.iter().map(|(a, _)| a.chars().count()).max().unwrap_or(0).min(24);
            for (i, (artist, n)) in self.artists.iter().enumerate() {
                let name: String = artist.chars().take(24).collect();
                rows.push(vec![
                    t(&format!("  {}. {name:<width$}  ", i + 1), if i == 0 { bright } else { dim }),
                    Seg::Bar { frac: *n as f32 / top as f32, width: 14, color: pal.petal },
                    t(&format!(" {n}"), dim),
                ]);
            }
            rows.push(vec![]);
        }
        rows.push(vec![t("Ánimo", accent)]);
        let top = self.moods.iter().map(|(_, n)| *n).max().unwrap_or(1).max(1);
        for (mood, n) in &self.moods {
            rows.push(vec![
                t(&format!("  {:<11} ", mood.name()), if *n == top { bright } else { dim }),
                Seg::Bar { frac: *n as f32 / top as f32, width: 14, color: mood_color(*mood) },
                t(&format!(" {n}"), dim),
            ]);
        }
        rows.push(vec![]);
        if let Some((avg, lo, hi)) = self.bpm {
            rows.push(vec![t("Tempo  ", accent), t(&format!("{avg:.0} BPM en promedio"), bright), t(&format!(" ({lo:.0}–{hi:.0})"), dim)]);
        }
        if let Some(key) = &self.key {
            rows.push(vec![t("Tonalidad  ", accent), t(key, bright), t(" la que más sonó", dim)]);
        }
        if let Some((title, artist, n)) = &self.top_song {
            rows.push(vec![t("La más repetida  ", accent), t(title, bright), t(&format!(" — {artist} ({n} veces)"), dim)]);
        }
        if let Some((day, n)) = &self.best_day {
            rows.push(vec![t("El día con más música  ", accent), t(&pretty_date(day), bright), t(&format!(" ({})", plural(*n, "canción", "canciones")), dim)]);
        }
        rows
    }
}

/// Un trozo de fila: texto o una barra de `width` caracteres llena hasta `frac`.
#[derive(Clone, Debug, PartialEq)]
pub enum Seg {
    Text(String, Rgb),
    Bar { frac: f32, width: usize, color: Rgb },
}

pub fn mood_color(mood: Mood) -> Rgb {
    match mood {
        Mood::Happy => Rgb(1.0, 0.82, 0.30),
        Mood::Serene => Rgb(1.0, 0.60, 0.40),
        Mood::Nostalgic => Rgb(0.65, 0.50, 1.0),
        Mood::Sad => Rgb(0.40, 0.60, 1.0),
        Mood::Intense => Rgb(1.0, 0.35, 0.35),
    }
}

/// El resumen como imagen, para compartir.
pub fn render(rows: &[Vec<Seg>], pal: &Palette) -> image::RgbImage {
    let scale = 2u32;
    let (cw, lh) = (8 * scale as i64, 12 * scale as i64);
    let cols = rows
        .iter()
        .map(|r| r.iter().map(|s| match s { Seg::Text(t, _) => t.chars().count(), Seg::Bar { width, .. } => *width }).sum::<usize>())
        .max()
        .unwrap_or(20)
        .max(36) as i64;
    let (w, h) = ((cols + 6) * cw, (rows.len() as i64 + 5) * lh);
    let mut img = image::RgbImage::new(w as u32, h as u32);
    for (y, row) in img.rows_mut().enumerate() {
        let c = pal.bg_top.mix(pal.bg_bottom, y as f32 / h as f32);
        let (r, g, b) = c.to_u8();
        for p in row {
            *p = image::Rgb([r, g, b]);
        }
    }
    for (i, row) in rows.iter().enumerate() {
        let mut x = 3 * cw;
        let y = (i as i64 + 2) * lh;
        // El título va más grande.
        let big = i == 0;
        for seg in row {
            match seg {
                Seg::Text(t, c) => {
                    // La fuente no tiene sostenidos ni bemoles: se escriben como # y b.
                    let t = t.replace('♯', "#").replace('♭', "b").replace('★', "*");
                    let s = if big { scale + 1 } else { scale };
                    text::draw(&mut img, x, y, &t, *c, s);
                    x += text::width(&t, s) as i64;
                }
                Seg::Bar { frac, width, color } => {
                    let full = (*width as i64 * cw) as f32;
                    let filled = (full * frac.clamp(0.0, 1.0)).round() as i64;
                    for px in 0..full as i64 {
                        let c = if px < filled { *color } else { pal.bg_top.mix(Rgb::WHITE, 0.12) };
                        let (r, g, b) = c.to_u8();
                        for py in 3..lh - 7 {
                            let (ix, iy) = (x + px, y + py);
                            if ix >= 0 && iy >= 0 && (ix as u32) < img.width() && (iy as u32) < img.height() {
                                img.put_pixel(ix as u32, iy as u32, image::Rgb([r, g, b]));
                            }
                        }
                    }
                    x += full as i64;
                }
            }
        }
    }
    let footer = "DiskTUI";
    text::draw_centered(&mut img, w / 2, h - 2 * lh, footer, pal.bg_top.mix(Rgb::WHITE, 0.35), scale);
    img
}

/// Guarda el resumen en `~/Pictures/DiskTUI/` y devuelve la ruta.
pub fn save(summary: &Summary, period: Period) -> Result<PathBuf> {
    if summary.songs == 0 {
        return Err(anyhow!("no hay canciones en {}", period.name()));
    }
    let pal = Palette::default();
    let path = crate::postcard::target("", &format!("Resumen {}", period.name()), "png")?;
    render(&summary.rows(period, &pal), &pal).save(&path).with_context(|| format!("no se pudo guardar {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plant::Genes;

    fn entry(date: &str, title: &str, artist: &str, mood: &str, bpm: f32) -> Entry {
        Entry {
            date: date.into(),
            title: title.into(),
            artist: artist.into(),
            bpm,
            key: 9,
            minor: true,
            mood: mood.into(),
            genes: Genes::from_song(&crate::features::Song::default()),
            petal: Rgb::WHITE,
            accent: Rgb::WHITE,
            stem: Rgb::WHITE,
        }
    }

    fn sample() -> Vec<Entry> {
        vec![
            entry("2026-11-19", "Uno", "Banda A", "alegre", 120.0),
            entry("2026-11-19", "Uno", "Banda A", "alegre", 122.0),
            entry("2026-11-18", "Dos", "Banda B", "triste", 80.0),
            entry("2026-11-15", "Tres", "Banda A", "serena", 100.0),
            entry("2026-10-30", "Cuatro", "Banda C", "intensa", 150.0),
            entry("2025-01-01", "Viejo", "Banda D", "nostálgica", 90.0),
        ]
    }

    #[test]
    fn counts_the_week_month_and_everything() {
        let all = sample();
        let week = Summary::of(&all, "2026-11-20", Period::Week, |_| false);
        assert_eq!((week.songs, week.distinct), (4, 3));
        assert_eq!(week.artists[0], ("Banda A".to_string(), 3));
        assert_eq!(week.top_song, Some(("Uno".into(), "Banda A".into(), 2)));
        assert_eq!(week.best_day, Some(("2026-11-19".into(), 2)));
        let (avg, lo, hi) = week.bpm.unwrap();
        assert!((avg - 105.5).abs() < 0.01 && lo == 80.0 && hi == 122.0);
        assert_eq!(week.key.as_deref(), Some("La menor"));
        assert_eq!(Summary::of(&all, "2026-11-20", Period::Month, |_| false).songs, 5);
        assert_eq!(Summary::of(&all, "2026-11-20", Period::All, |_| false).songs, 6);
    }

    #[test]
    fn moods_and_favorites() {
        let all = sample();
        let s = Summary::of(&all, "2026-11-20", Period::Week, |e| e.title == "Dos");
        let happy = s.moods.iter().find(|(m, _)| *m == Mood::Happy).unwrap().1;
        assert_eq!(happy, 2);
        assert_eq!(s.favorites, 1);
    }

    #[test]
    fn empty_periods_say_so_and_render_anyway() {
        let s = Summary::of(&[], "2026-11-20", Period::Week, |_| false);
        assert_eq!(s.songs, 0);
        let rows = s.rows(Period::Week, &Palette::default());
        assert!(rows.len() >= 3);
        let img = render(&rows, &Palette::default());
        assert!(img.width() > 100 && img.height() > 50);
        let full = Summary::of(&sample(), "2026-11-20", Period::All, |_| false);
        let img = render(&full.rows(Period::All, &Palette::default()), &Palette::default());
        assert!(img.height() > 300);
    }

    #[test]
    fn dates_read_like_people_say_them() {
        assert_eq!(pretty_date("2026-11-14"), "sábado 14 de noviembre");
        assert_eq!(pretty_date("2026-11-20"), "viernes 20 de noviembre");
        assert_eq!(pretty_date("1970-01-01"), "jueves 1 de enero");
        assert_eq!(pretty_date("cualquier cosa"), "cualquier cosa");
    }

    #[test]
    fn periods_cycle() {
        assert_eq!(Period::Week.step(1), Period::Month);
        assert_eq!(Period::Week.step(-1), Period::All);
    }
}

//! Herbario: cada canción escuchada casi entera deja su flor guardada.
//!
//! Se guarda en `~/.local/share/disktui/herbario.tsv`, una canción por línea. Con
//! `disktui --herbario` (o `g` en el menú) se ve la colección.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use ratatui::DefaultTerminal;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Span;

use crate::canvas::Canvas;
use crate::color::{ColorMode, Rgb};
use crate::features::{NOTES, Song};
use crate::mood::Mood;
use crate::palette::Palette;
use crate::plant::{Genes, Plant};
use crate::render::{self, View};

/// Ancho y alto de cada ficha, en celdas.
const TILE: (usize, usize) = (30, 12);

/// Una flor guardada.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub date: String,
    pub title: String,
    pub artist: String,
    pub bpm: f32,
    pub key: u8,
    pub minor: bool,
    pub mood: String,
    pub genes: Genes,
    pub petal: Rgb,
    pub accent: Rgb,
    pub stem: Rgb,
}

fn hex(c: Rgb) -> String {
    let (r, g, b) = c.to_u8();
    format!("{r:02x}{g:02x}{b:02x}")
}

fn unhex(s: &str) -> Option<Rgb> {
    let n = u32::from_str_radix(s.trim(), 16).ok()?;
    Some(Rgb::from_u8((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

/// Las tabulaciones y saltos de línea separan campos y fichas: en los textos se cambian.
fn clean(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
}

impl Entry {
    /// Arma la ficha de la canción que acaba de sonar.
    pub fn new(title: &str, artist: &str, song: &Song, genes: Genes, pal: &Palette) -> Entry {
        Entry {
            date: today(),
            title: clean(title),
            artist: clean(artist),
            bpm: song.bpm,
            key: song.key,
            minor: song.minor,
            mood: Mood::nearest(song.valence, song.arousal).name().to_string(),
            genes,
            petal: pal.petal,
            accent: pal.accent,
            stem: pal.stem,
        }
    }

    fn to_line(&self) -> String {
        format!(
            "{}\t{}\t{}\t{:.1}\t{}\t{}\t{}\t{}\t{:.3}\t{}\t{:.3}\t{:.4}\t{}\t{}\t{}",
            self.date,
            self.title,
            self.artist,
            self.bpm,
            self.key,
            self.minor as u8,
            self.mood,
            self.genes.petals,
            self.genes.sharp,
            self.genes.layers,
            self.genes.size,
            self.genes.hue,
            hex(self.petal),
            hex(self.accent),
            hex(self.stem),
        )
    }

    fn from_line(line: &str) -> Option<Entry> {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 15 {
            return None;
        }
        Some(Entry {
            date: f[0].to_string(),
            title: f[1].to_string(),
            artist: f[2].to_string(),
            bpm: f[3].parse().ok()?,
            key: f[4].parse::<u8>().ok()? % 12,
            minor: f[5] == "1",
            mood: f[6].to_string(),
            genes: Genes {
                petals: f[7].parse::<u32>().ok()?.clamp(3, 12),
                sharp: f[8].parse().ok()?,
                layers: f[9].parse().ok()?,
                size: f[10].parse().ok()?,
                hue: f[11].parse().ok()?,
            },
            petal: unhex(f[12])?,
            accent: unhex(f[13])?,
            stem: unhex(f[14])?,
        })
    }

    fn info(&self) -> String {
        let key = format!("{} {}", NOTES[self.key as usize % 12], if self.minor { "menor" } else { "mayor" });
        if self.bpm > 0.0 {
            format!("{:.0} BPM · {key} · {}", self.bpm, self.mood)
        } else {
            format!("{key} · {}", self.mood)
        }
    }
}

/// Fecha local de hoy, `AAAA-MM-DD`. Usa el comando `date` para respetar la zona horaria.
fn today() -> String {
    std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".local").join("share").join("disktui").join("herbario.tsv"))
}

pub fn parse(text: &str) -> Vec<Entry> {
    text.lines().filter_map(Entry::from_line).collect()
}

pub fn load() -> Vec<Entry> {
    path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| parse(&t)).unwrap_or_default()
}

/// Agrega la ficha. Si la misma canción ya estaba, la reemplaza y la deja al final.
pub fn merge(mut entries: Vec<Entry>, entry: Entry) -> Vec<Entry> {
    entries.retain(|e| !(e.title == entry.title && e.artist == entry.artist));
    entries.push(entry);
    entries
}

/// Guarda una flor en el herbario. Si falla (sin permisos), no pasa nada: es un extra.
pub fn record(entry: Entry) {
    let Some(file) = path() else { return };
    let entries = merge(load(), entry);
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let text: String = entries.iter().map(|e| e.to_line() + "\n").collect();
    let _ = std::fs::write(file, text);
}

/// Dibuja la flor de una ficha centrada en (`cx`, `cy`) píxeles, con radio `radius`.
pub fn draw_specimen(canvas: &mut Canvas, entry: &Entry, cx: f32, cy: f32, radius: f32, time: f32) {
    let pal = Palette {
        petal: entry.petal,
        petal_light: entry.petal.mix(Rgb::WHITE, 0.4),
        accent: entry.accent,
        stem: entry.stem,
        leaf: entry.stem.mix(Rgb::WHITE, 0.2),
        ..Palette::default()
    };
    // Una planta ya crecida y abierta, con los rasgos guardados.
    let mut plant = Plant::new(1, None);
    plant.growth = 1.0;
    plant.open = 1.0;
    plant.genes = Some(entry.genes);
    plant.rot = time * 0.15;
    let flower = plant.flower();
    let scale = radius / flower.radius.max(1e-3);
    let view = View { cx: cx - flower.x * scale, ground: cy + flower.y * scale, scale, weight: 0.5 };
    // Tallo corto bajo la flor.
    canvas.line(cx * 2.0, (cy + radius * 0.6) * 2.0, cx * 2.0 + 1.0, (cy + radius * 2.1) * 2.0, entry.stem.mix(Rgb::WHITE, 0.15));
    render::draw_flower(canvas, &view, &plant, &flower, &pal);
}

/// Pantalla del herbario. Vuelve al salir con `Esc` o `q`.
pub fn view(terminal: &mut DefaultTerminal, mode: ColorMode) -> Result<()> {
    let entries = load();
    let pal = Palette::default();
    let mut canvas = Canvas::new(1, 1);
    let mut scroll = 0usize;
    let start = Instant::now();
    loop {
        let size = terminal.size()?;
        let cols = (size.width as usize / TILE.0).max(1);
        let rows_visible = (size.height as usize).saturating_sub(2) / TILE.1;
        let total_rows = entries.len().div_ceil(cols);
        let max_scroll = total_rows.saturating_sub(rows_visible.max(1));
        scroll = scroll.min(max_scroll);

        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('g') | KeyCode::Enter => return Ok(()),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
                    KeyCode::Up => scroll = scroll.saturating_sub(1),
                    KeyCode::Down => scroll = (scroll + 1).min(max_scroll),
                    _ => {}
                },
                // Aquí el scroll sí sirve: desplaza la colección.
                Event::Mouse(m) => match m.kind {
                    MouseEventKind::ScrollUp => scroll = scroll.saturating_sub(1),
                    MouseEventKind::ScrollDown => scroll = (scroll + 1).min(max_scroll),
                    _ => {}
                },
                _ => {}
            }
        }

        let time = start.elapsed().as_secs_f32();
        canvas.resize(size.width as usize, size.height as usize);
        let ph = canvas.px_h().max(1) as f32;
        canvas.clear(|_, y| pal.bg_top.mix(pal.bg_bottom, y as f32 / ph));
        // Lo más nuevo primero.
        let shown: Vec<(usize, &Entry)> = entries.iter().rev().enumerate().skip(scroll * cols).take(rows_visible.max(1) * cols).collect();
        let margin = (size.width as usize).saturating_sub(cols * TILE.0) / 2;
        for (i, entry) in &shown {
            let (col, row) = (i % cols, i / cols - scroll);
            let x0 = margin + col * TILE.0;
            let y0 = 1 + row * TILE.1;
            let cx = x0 as f32 + TILE.0 as f32 / 2.0;
            let cy = (y0 as f32 + 3.6) * 2.0;
            draw_specimen(&mut canvas, entry, cx, cy, 5.5, time + *i as f32);
        }

        terminal.draw(|f| {
            let area = f.area();
            let buf = f.buffer_mut();
            canvas.render(buf, area, mode);
            let dim = pal.bg_top.mix(Rgb::WHITE, 0.45);
            let bright = pal.bg_top.mix(Rgb::WHITE, 0.9);
            let header = format!(" ✿ Herbario · {} flores", entries.len());
            put(buf, area, mode, 0, 0, &header, pal.petal.mix(Rgb::WHITE, 0.3), area.width as usize);
            if entries.is_empty() {
                let text = "Aún no hay flores. Escucha una canción casi entera y quedará guardada.";
                put(buf, area, mode, 2, area.height / 2, text, dim, area.width as usize - 2);
            }
            for (i, entry) in &shown {
                let (col, row) = (i % cols, i / cols - scroll);
                let x0 = (margin + col * TILE.0) as u16;
                let y0 = (1 + row * TILE.1) as u16;
                for (k, (text, color)) in
                    [(&entry.title, bright), (&entry.artist, dim), (&entry.info(), dim), (&entry.date, dim.mix(pal.bg_top, 0.4))]
                        .into_iter()
                        .enumerate()
                {
                    let w = Span::raw(text.as_str()).width().min(TILE.0 - 2);
                    let x = x0 + ((TILE.0 - w) / 2) as u16;
                    put(buf, area, mode, x, y0 + 8 + k as u16, text, color, TILE.0 - 2);
                }
            }
            let footer = " ↑↓ o scroll para desplazar · Esc para volver";
            put(buf, area, mode, 0, area.height.saturating_sub(1), footer, dim, area.width as usize);
        })?;
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Escribe texto recortado a `max` columnas, sin tocar el fondo.
#[allow(clippy::too_many_arguments)]
fn put(buf: &mut Buffer, clip: Rect, mode: ColorMode, x: u16, y: u16, text: &str, fg: Rgb, max: usize) {
    let mut col = x;
    let mut used = 0;
    let total = Span::raw(text).width();
    for ch in text.chars() {
        let w = Span::raw(ch.to_string()).width().max(1);
        if col >= clip.right() || y >= clip.bottom() {
            break;
        }
        // Si no cabe entero, el último lugar es para los puntos suspensivos.
        let last = total > max && used + w >= max;
        let cell = &mut buf[(col, y)];
        cell.set_char(if last { '…' } else { ch });
        cell.fg = mode.color(fg);
        if last {
            break;
        }
        col += w as u16;
        used += w;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(title: &str) -> Entry {
        let song = Song { bpm: 96.4, key: 9, minor: true, valence: 0.48, arousal: 0.44, majorness: -0.6, ..Song::default() };
        let mut e = Entry::new(title, "Una\tBanda", &song, Genes::from_song(&song), &Palette::default());
        e.date = "2026-11-11".into();
        e
    }

    #[test]
    fn entries_roundtrip_through_the_file_format() {
        let e = sample("Canción con\ttab y ñ");
        let back = parse(&(e.to_line() + "\n"));
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].title, "Canción con tab y ñ");
        assert_eq!(back[0].artist, "Una Banda");
        assert_eq!(back[0].genes.petals, e.genes.petals);
        assert_eq!(back[0].mood, "nostálgica");
        assert!(back[0].petal.dist2(e.petal) < 1e-4);
        assert_eq!(back[0].info(), "96 BPM · La menor · nostálgica");
    }

    #[test]
    fn broken_lines_are_skipped() {
        let text = format!("basura\n{}\n\ncasi\tuna\tficha\n", sample("A").to_line());
        assert_eq!(parse(&text).len(), 1);
    }

    #[test]
    fn same_song_is_replaced_not_duplicated() {
        let list = merge(merge(merge(Vec::new(), sample("A")), sample("B")), sample("A"));
        assert_eq!(list.iter().map(|e| e.title.as_str()).collect::<Vec<_>>(), ["B", "A"]);
    }

    #[test]
    fn specimen_draws_inside_its_tile() {
        let mut canvas = Canvas::new(TILE.0, TILE.1);
        canvas.clear(|_, _| Rgb::BLACK);
        draw_specimen(&mut canvas, &sample("A"), 15.0, 7.2, 5.5, 0.0);
        // El centro de la flor quedó pintado.
        assert!(canvas.covered(15, 3));
    }
}

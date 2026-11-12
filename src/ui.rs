//! HUD al pie: título, artista, fuente y una barra de reproducción con la forma de la canción.
//!
//! Con espacio usa tres filas: una de texto y dos para la barra, que dibuja la intensidad de
//! la canción como una onda (estilo SoundCloud). En modo archivo la onda se conoce entera
//! desde el inicio, con los clímax marcados. En Spotify y demo se va dibujando al sonar.
//! En terminales bajas queda una sola línea compacta.

use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;

use crate::analysis::Bands;
use crate::color::{ColorMode, Rgb};
use crate::mood::Mood;
use crate::palette::Palette;
use crate::scene::SceneKind;
use crate::session::Session;
use crate::structure::SongMap;

fn clock(d: Duration) -> String {
    let s = d.as_secs();
    format!("{}:{:02}", s / 60, s % 60)
}

/// Recorta `text` a `max` columnas, con puntos suspensivos si no cabe.
fn fit(text: &str, max: usize) -> String {
    if Span::raw(text).width() <= max {
        return text.to_string();
    }
    let mut out = String::new();
    for ch in text.chars() {
        out.push(ch);
        if Span::raw(out.as_str()).width() + 1 > max {
            out.pop();
            break;
        }
    }
    out.push('…');
    out
}

/// Resolución de la línea de tiempo: tramos en que se divide la canción.
const BINS: usize = 720;
/// Bloques de un octavo a celda completa.
const BLOCKS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Forma de la canción a lo largo del tiempo, para la barra de reproducción.
pub struct Timeline {
    /// Intensidad 0..1 por tramo; `None` = aún no se conoce.
    bins: Vec<Option<f32>>,
    /// Posición de cada clímax, como fracción 0..1 de la canción.
    climaxes: Vec<f32>,
    /// La canción completa ya está cargada (modo archivo).
    complete: bool,
}

impl Timeline {
    pub fn new() -> Self {
        Timeline { bins: vec![None; BINS], climaxes: Vec::new(), complete: false }
    }

    /// Canción nueva: se borra todo.
    pub fn reset(&mut self) {
        *self = Timeline::new();
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Carga la canción entera desde el pre-análisis del archivo.
    pub fn load_map(&mut self, map: &SongMap, duration: f32) {
        if duration <= 0.0 {
            return;
        }
        for (i, bin) in self.bins.iter_mut().enumerate() {
            *bin = Some(map.intensity_at((i as f32 + 0.5) / BINS as f32 * duration));
        }
        self.climaxes = map.climaxes.iter().map(|t| (t / duration).clamp(0.0, 1.0)).collect();
        self.complete = true;
    }

    /// Anota lo que suena ahora. Sin efecto si la canción ya está cargada entera.
    pub fn record(&mut self, progress: f32, intensity: f32, climax: bool) {
        if self.complete {
            return;
        }
        let i = ((progress.clamp(0.0, 1.0) * BINS as f32) as usize).min(BINS - 1);
        self.bins[i] = Some(self.bins[i].map_or(intensity, |v| v.max(intensity)));
        if climax {
            self.climaxes.push(progress);
        }
    }

    /// Tramos que cubre la columna `c` de una barra de `width` columnas.
    fn span(c: usize, width: usize) -> (usize, usize) {
        let a = c * BINS / width.max(1);
        let b = ((c + 1) * BINS / width.max(1)).clamp(a + 1, BINS);
        (a.min(BINS - 1), b)
    }

    /// Intensidad media de una columna; `None` si nada de ella se conoce.
    fn column(&self, c: usize, width: usize) -> Option<f32> {
        let (a, b) = Self::span(c, width);
        let known: Vec<f32> = self.bins[a..b].iter().flatten().copied().collect();
        (!known.is_empty()).then(|| known.iter().sum::<f32>() / known.len() as f32)
    }

    fn climax_in(&self, c: usize, width: usize) -> bool {
        let (a, b) = Self::span(c, width);
        self.climaxes.iter().any(|f| {
            let i = ((f * BINS as f32) as usize).min(BINS - 1);
            i >= a && i < b
        })
    }
}

/// Todo lo que el HUD necesita para dibujarse.
pub struct Hud<'a> {
    pub session: &'a dyn Session,
    pub pal: &'a Palette,
    pub mode: ColorMode,
    pub timeline: &'a Timeline,
    pub bands: &'a Bands,
    pub scene: SceneKind,
    /// Aviso pasajero (postal guardada, flor al herbario): reemplaza el texto de la derecha.
    pub notice: Option<&'a str>,
}

/// Filas que ocupa el HUD: 3 con espacio, 1 en terminales bajas, 0 oculto.
pub fn rows(show: bool, terminal_height: u16) -> u16 {
    if !show || terminal_height <= 6 {
        0
    } else if terminal_height < 16 {
        1
    } else {
        3
    }
}

pub fn draw(buf: &mut Buffer, area: Rect, hud: &Hud) {
    if area.height >= 3 && area.width >= 40 {
        draw_full(buf, area, hud);
    } else if area.height >= 1 {
        let line = Rect { y: area.bottom() - 1, height: 1, ..area };
        draw_compact(buf, line, hud.session, hud.pal, hud.mode);
    }
}

/// Escribe `text` desde la columna `x` de la fila `y`, sin tocar el fondo de las celdas.
fn put(buf: &mut Buffer, area: Rect, mode: ColorMode, x: usize, y: u16, text: &str, fg: Rgb) -> usize {
    let mut col = x;
    for ch in text.chars() {
        if col >= area.width as usize {
            break;
        }
        let cell = &mut buf[(area.x + col as u16, area.y + y)];
        cell.set_char(ch);
        cell.fg = mode.color(fg);
        col += Span::raw(ch.to_string()).width().max(1);
    }
    col
}

fn draw_full(buf: &mut Buffer, area: Rect, hud: &Hud) {
    let (pal, mode, session) = (hud.pal, hud.mode, hud.session);
    let width = area.width as usize;
    let bg = pal.bg_bottom.scale(0.55);
    let dim = bg.mix(Rgb::WHITE, 0.42);
    let bright = bg.mix(Rgb::WHITE, 0.88);
    for y in 0..3 {
        for x in 0..area.width {
            buf[(area.x + x, area.y + y)].set_char(' ').set_style(Style::new().bg(mode.color(bg)));
        }
    }

    // Fila 1: título y artista a la izquierda; escena, fuente y volumen a la derecha.
    let track = session.track();
    let (right, right_color) = match hud.notice.map(str::to_string).or_else(|| session.warning()) {
        Some(warning) => (warning, pal.accent),
        None => {
            let scene = hud.scene.name();
            let mut text = String::new();
            // Lo que se entendió de la canción: tempo, tonalidad y ánimo.
            let song = &hud.bands.song;
            if song.settled > 0.35 {
                if song.tempo_confidence > 0.25 && song.bpm > 0.0 {
                    text.push_str(&format!("{:.0} BPM · ", song.bpm));
                }
                if song.key_confidence > 0.25 {
                    text.push_str(&format!("{} · ", song.key_name()));
                }
                text.push_str(Mood::nearest(song.valence, song.arousal).name());
                text.push_str("  │  ");
            } else if session.playing() && hud.scene == SceneKind::Garden {
                text.push_str("escuchando…  │  ");
            }
            text.push_str(&format!("{scene} · {}", session.label()));
            if let Some(v) = session.volume() {
                text.push_str(&format!(" · vol {:.0}%", v * 100.0));
            }
            (text, dim)
        }
    };
    let right = fit(&right, width * 2 / 3);
    let right_w = Span::raw(right.as_str()).width();
    let room = width.saturating_sub(right_w + 6);
    let title = fit(&track.title, room);
    let title_w = Span::raw(title.as_str()).width();
    put(buf, area, mode, 1, 0, "♪", pal.petal.mix(Rgb::WHITE, 0.2));
    let end = put(buf, area, mode, 3, 0, &title, bright);
    if !track.artist.is_empty() && room > title_w + 6 {
        put(buf, area, mode, end, 0, &format!(" — {}", fit(&track.artist, room - title_w - 3)), dim);
    }
    put(buf, area, mode, width.saturating_sub(right_w + 1), 0, &right, right_color);

    // Filas 2 y 3: estado y tiempo, la onda, duración y medidores de banda.
    let elapsed = clock(session.position());
    let total = track.duration.map(clock).unwrap_or_else(|| "-:--".into());
    let icon = if session.playing() { "▶" } else { "⏸" };
    put(buf, area, mode, 1, 2, icon, pal.petal.mix(Rgb::WHITE, 0.3));
    let wave_x = put(buf, area, mode, 3, 2, &elapsed, bright) + 1;
    let meters_w = 11; // "G█ M█ A█" más márgenes
    let wave_end = width.saturating_sub(total.len() + meters_w + 3);
    put(buf, area, mode, wave_end + 1, 2, &total, dim);
    let b = hud.bands;
    let mut x = wave_end + total.len() + 3;
    for (label, value, color) in [("G", b.bass, pal.petal), ("M", b.mid, pal.accent), ("A", b.treble, pal.petal_light)] {
        x = put(buf, area, mode, x, 2, label, dim);
        let level = (value.clamp(0.0, 1.0) * 8.0).round() as usize;
        x = put(buf, area, mode, x, 2, &BLOCKS[level.max(1)].to_string(), color) + 1;
    }

    if wave_end <= wave_x + 4 {
        return;
    }
    let wave_w = wave_end - wave_x;
    let progress = session.progress();
    let head = progress.map(|p| ((p * wave_w as f32) as usize).min(wave_w - 1));
    for c in 0..wave_w {
        let value = hud.timeline.column(c, wave_w);
        let played = head.is_some_and(|h| c < h);
        let is_head = head == Some(c);
        let climax = hud.timeline.climax_in(c, wave_w);
        // Altura en dieciseisavos: dos filas de ocho niveles.
        let mut level = match value {
            Some(v) => 1 + (v.clamp(0.0, 1.0) * 15.0).round() as usize,
            None => 1,
        };
        if is_head {
            level = level.max(5);
        }
        let along = c as f32 / wave_w as f32;
        let color = if is_head {
            Rgb::WHITE
        } else if played {
            let base = pal.petal.mix(pal.accent, along);
            if climax { base.mix(Rgb::WHITE, 0.55) } else { base }
        } else if climax {
            // Clímax por venir: se asoma en el color de acento.
            bg.mix(pal.accent, 0.55)
        } else if value.is_some() {
            bg.mix(Rgb::WHITE, 0.20)
        } else {
            bg.mix(Rgb::WHITE, 0.10)
        };
        let (upper, lower) = if level > 8 { (BLOCKS[level - 8], '█') } else { (' ', BLOCKS[level]) };
        for (row, ch) in [(1u16, upper), (2u16, lower)] {
            let cell = &mut buf[(area.x + (wave_x + c) as u16, area.y + row)];
            cell.set_char(ch);
            cell.fg = mode.color(color);
        }
    }
}

/// HUD de una línea, para terminales bajas.
fn draw_compact(buf: &mut Buffer, area: Rect, session: &dyn Session, pal: &Palette, mode: ColorMode) {
    if area.height == 0 || area.width < 8 {
        return;
    }
    let width = area.width as usize;
    let bg = pal.bg_bottom.scale(0.7);
    let dim = bg.mix(Rgb::WHITE, 0.45);
    let bright = bg.mix(Rgb::WHITE, 0.85);

    // El fondo de la línea se aclara hasta donde va la canción: barra de progreso discreta.
    let filled = session.progress().map(|p| (p * width as f32) as usize).unwrap_or(0);
    for x in 0..width {
        let cell_bg = if x < filled { bg.mix(pal.petal, 0.22) } else { bg };
        buf[(area.x + x as u16, area.y)].set_char(' ').set_style(Style::new().bg(mode.color(cell_bg)));
    }

    // Derecha: aviso o tiempo, estado, volumen y fuente.
    let track = session.track();
    let mut right = String::new();
    if let Some(warning) = session.warning() {
        right.push_str(&warning);
    } else {
        right.push_str(&clock(session.position()));
        if let Some(total) = track.duration {
            right.push_str(&format!(" / {}", clock(total)));
        }
        right.push_str(if session.playing() { "  ▶" } else { "  ⏸" });
        if let Some(v) = session.volume() {
            right.push_str(&format!("  vol {:.0}%", v * 100.0));
        }
        right.push_str(&format!("  {}", session.label()));
    }
    let right = fit(&right, width.saturating_sub(2) / 2 + 8);
    let right_w = Span::raw(right.as_str()).width();

    // Izquierda: título y artista, en el espacio que quede.
    let room = width.saturating_sub(right_w + 5);
    let title = fit(&track.title, room);
    let title_w = Span::raw(title.as_str()).width();
    let artist = if track.artist.is_empty() || room < title_w + 6 {
        String::new()
    } else {
        format!(" — {}", fit(&track.artist, room - title_w - 3))
    };

    // Cada texto conserva el fondo de progreso de sus celdas.
    let mut put = |x: usize, text: &str, fg: Rgb| {
        let mut col = x;
        for ch in text.chars() {
            if col >= width {
                break;
            }
            let cell = &mut buf[(area.x + col as u16, area.y)];
            cell.set_char(ch);
            cell.fg = mode.color(fg);
            col += Span::raw(ch.to_string()).width().max(1);
        }
    };
    put(1, "♪", pal.petal.mix(Rgb::WHITE, 0.2));
    put(3, &title, bright);
    put(3 + title_w, &artist, dim);
    put(width.saturating_sub(right_w + 1), &right, dim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_truncates_with_ellipsis() {
        assert_eq!(fit("hola", 10), "hola");
        assert_eq!(fit("una canción muy larga", 8), "una can…");
    }

    #[test]
    fn rows_depend_on_height() {
        assert_eq!(rows(false, 50), 0);
        assert_eq!(rows(true, 5), 0);
        assert_eq!(rows(true, 12), 1);
        assert_eq!(rows(true, 40), 3);
    }

    #[test]
    fn timeline_fills_as_the_song_plays() {
        let mut t = Timeline::new();
        assert_eq!(t.column(10, 100), None);
        t.record(0.105, 0.8, false);
        t.record(0.5, 0.4, true);
        assert!((t.column(10, 100).unwrap() - 0.8).abs() < 1e-5);
        assert_eq!(t.column(11, 100), None);
        assert!(t.climax_in(50, 100) && !t.climax_in(49, 100));
        t.reset();
        assert_eq!(t.column(10, 100), None);
    }

    #[test]
    fn timeline_loads_the_whole_song_from_a_map() {
        // 100 s: suave, fuerte desde el segundo 40, suave desde el 70.
        let db: Vec<f32> = (0..400).map(|k| if (160..280).contains(&k) { -10.0 } else { -25.0 }).collect();
        let map = SongMap::from_loudness(&db);
        let mut t = Timeline::new();
        t.load_map(&map, 100.0);
        assert!(t.is_complete());
        assert!(t.column(20, 100).unwrap() < 0.2 && t.column(55, 100).unwrap() > 0.8);
        assert!((38..=42).any(|c| t.climax_in(c, 100)), "clímax cerca de la columna 40");
        // Con la canción cargada, lo que suena en vivo no la pisa.
        t.record(0.2, 1.0, false);
        assert!(t.column(20, 100).unwrap() < 0.2);
    }

    #[test]
    fn full_hud_draws_at_many_widths() {
        use crate::demo::DemoSession;
        let session = DemoSession::new(60.0);
        let (pal, timeline, bands) = (Palette::default(), Timeline::new(), Bands::default());
        for width in [8u16, 39, 40, 41, 60, 200] {
            let area = Rect::new(0, 0, width, 3);
            let mut buf = Buffer::empty(area);
            let hud = Hud { session: &session, pal: &pal, mode: ColorMode::True, timeline: &timeline, bands: &bands, scene: SceneKind::Garden, notice: None };
            draw(&mut buf, area, &hud);
        }
    }

    #[test]
    fn clock_format() {
        assert_eq!(clock(Duration::from_secs(215)), "3:35");
    }
}

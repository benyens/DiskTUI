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
use crate::features::Song;
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
    /// La escena la eligió la canción (modo automático).
    pub auto_scene: bool,
    /// La canción que suena está marcada como favorita: la nota se vuelve estrella.
    pub favorite: bool,
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
        draw_compact(buf, line, hud.session, hud.pal, hud.mode, hud.favorite);
    }
}

/// Lo que muestra el panel de análisis (tecla `d`).
pub struct Analysis<'a> {
    pub song: &'a Song,
    /// Ánimo en uso y de dónde sale: "auto", "corregido" o "fijo".
    pub mood: Mood,
    pub origin: &'a str,
    /// Ánimo del momento, movido por la sección que suena.
    pub moment: Mood,
    /// De dónde viene el audio, por ejemplo "Spotify · solo la app".
    pub source: String,
    /// Aviso completo de la fuente, si hay.
    pub warning: Option<String>,
    pub pal: &'a Palette,
    pub mode: ColorMode,
}

const PANEL_WIDTH: u16 = 46;
const PANEL_HEIGHT: u16 = 10;
/// Líneas máximas para el aviso dentro del panel.
const PANEL_WARNING_LINES: usize = 4;

/// Corta `text` en líneas de a lo más `width` caracteres, por palabras.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.chars().take(width).collect()),
        }
    }
    lines
}

/// Panel de análisis, arriba a la derecha: qué entendió DiskTUI de la canción y cómo
/// corregirlo. No se dibuja si no cabe.
pub fn draw_analysis(buf: &mut Buffer, area: Rect, a: &Analysis) {
    let mut notes = a.warning.as_deref().map(|w| wrap(w, PANEL_WIDTH as usize - 2)).unwrap_or_default();
    notes.truncate(PANEL_WARNING_LINES);
    let height = PANEL_HEIGHT + notes.len() as u16 + u16::from(!notes.is_empty());
    if area.width < PANEL_WIDTH + 2 || area.height < height + 1 {
        return;
    }
    let panel = Rect { x: area.right() - PANEL_WIDTH - 1, y: area.y + 1, width: PANEL_WIDTH, height };
    let bg = a.pal.bg_bottom.scale(0.45);
    let dim = bg.mix(Rgb::WHITE, 0.45);
    let bright = bg.mix(Rgb::WHITE, 0.90);
    for y in 0..panel.height {
        for x in 0..panel.width {
            buf[(panel.x + x, panel.y + y)].set_char(' ').set_style(Style::new().bg(a.mode.color(bg)));
        }
    }
    let song = a.song;
    let bar = |v: f32| {
        let filled = (v.clamp(0.0, 1.0) * 12.0).round() as usize;
        format!("{}{}", "█".repeat(filled), "░".repeat(12 - filled))
    };
    let percent = |v: f32| format!("{:.0}%", v.clamp(0.0, 1.0) * 100.0);
    let tempo = if song.bpm > 0.0 { format!("{:.0} BPM · confianza {}", song.bpm, percent(song.tempo_confidence)) } else { "escuchando…".into() };
    let key = if song.settled > 0.2 { format!("{} · confianza {}", song.key_name(), percent(song.key_confidence)) } else { "escuchando…".into() };
    let rows: [(&str, String); 6] = [
        ("ánimo", if a.moment == a.mood { format!("{} ({})", a.mood.name(), a.origin) } else { format!("{} ({}) · ahora {}", a.mood.name(), a.origin, a.moment.name()) }),
        ("valencia", format!("{} {:.2}", bar(song.valence), song.valence)),
        ("activación", format!("{} {:.2}", bar(song.arousal), song.arousal)),
        ("tempo", tempo),
        ("tonalidad", key),
        ("timbre", format!("brillo {:.2} · densidad {:.2}", song.brightness, song.density)),
    ];
    let end = put(buf, panel, a.mode, 1, 0, "análisis", a.pal.petal.mix(Rgb::WHITE, 0.3));
    put(buf, panel, a.mode, end, 0, &format!(" · escuchado {}", percent(song.settled)), dim);
    for (i, (label, value)) in rows.iter().enumerate() {
        put(buf, panel, a.mode, 1, 1 + i as u16, label, dim);
        put(buf, panel, a.mode, 13, 1 + i as u16, value, bright);
    }
    put(buf, panel, a.mode, 1, 7, "fuente", dim);
    put(buf, panel, a.mode, 13, 7, &a.source, bright);
    put(buf, panel, a.mode, 1, 8, "corregir: 1 alegre  2 serena  3 nostálgica", dim);
    put(buf, panel, a.mode, 1, 9, "          4 triste  5 intensa  0 auto", dim);
    for (i, line) in notes.iter().enumerate() {
        put(buf, panel, a.mode, 1, PANEL_HEIGHT + 1 + i as u16, line, a.pal.accent);
    }
}

/// Tarjeta de canción: título y artista grandes al empezar cada tema, que se desvanecen.
pub struct Card<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    /// 0 invisible, 1 del todo.
    pub alpha: f32,
    pub pal: &'a Palette,
    pub mode: ColorMode,
}

/// Cuánto dura la tarjeta, en segundos: entra, se queda y se va.
pub const CARD_SECONDS: f32 = 4.8;

/// Opacidad de la tarjeta `t` segundos después de empezar la canción.
pub fn card_alpha(t: f32) -> f32 {
    let fade_in = (t / 0.7).clamp(0.0, 1.0);
    let fade_out = ((CARD_SECONDS - t) / 1.2).clamp(0.0, 1.0);
    let a = fade_in.min(fade_out);
    a * a * (3.0 - 2.0 * a)
}

/// Filas (en celdas) de la franja de la tarjeta dentro de un área de `height` filas, o
/// `None` si no cabe.
pub fn card_band(height: u16) -> Option<(u16, u16)> {
    if height < 9 {
        return None;
    }
    let center = (height as f32 * 0.66) as u16;
    Some((center.saturating_sub(2), (center + 3).min(height)))
}

/// Título con letras separadas si cabe, como en un videoclip; si no, tal cual y recortado.
fn card_title(title: &str, width: usize) -> String {
    let upper = title.to_uppercase();
    let spaced: String = upper.chars().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");
    let fits = |t: &str| t.chars().count() + 4 <= width;
    if fits(&spaced) {
        spaced
    } else if fits(&upper) {
        upper
    } else {
        let room = width.saturating_sub(5);
        format!("{}…", upper.chars().take(room).collect::<String>().trim_end())
    }
}

/// Dibuja la tarjeta sobre la franja ya oscurecida del lienzo.
pub fn draw_card(buf: &mut Buffer, area: Rect, card: &Card) {
    let Some((top, _)) = card_band(area.height) else { return };
    if card.alpha <= 0.01 || card.title.trim().is_empty() {
        return;
    }
    let width = area.width as usize;
    let back = card.pal.bg_bottom.scale(0.3);
    let fade = |c: Rgb| back.mix(c, card.alpha);
    let title = card_title(card.title, width);
    let line: String = "━".repeat(title.chars().count().clamp(6, 18) / 3);
    let center = |text: &str| width.saturating_sub(text.chars().count()) / 2;
    put(buf, area, card.mode, center(&line), top, &line, fade(card.pal.accent));
    put(buf, area, card.mode, center(&title), top + 1, &title, fade(Rgb::WHITE));
    if !card.artist.trim().is_empty() {
        let artist = text_fit(card.artist, width.saturating_sub(4));
        put(buf, area, card.mode, center(&artist), top + 3, &artist, fade(card.pal.petal_light.mix(Rgb::WHITE, 0.4)));
    }
}

/// La letra que suena y la que viene.
pub struct LyricsLine<'a> {
    /// Arriba del lienzo en vez de abajo.
    pub top: bool,
    pub current: &'a str,
    pub next: &'a str,
    /// Cuánto de la línea actual ya se cantó, de 0 a 1: se ilumina de a poco.
    pub progress: f32,
    pub pal: &'a Palette,
    pub mode: ColorMode,
}

/// Filas (en celdas) de la franja de la letra, o `None` si no cabe. Va abajo, salvo en
/// las escenas que tienen lo importante abajo (`top`).
pub fn lyrics_band(height: u16, top: bool) -> Option<(u16, u16)> {
    match (height >= 8, top) {
        (false, _) => None,
        (true, true) => Some((0, 3)),
        (true, false) => Some((height - 4, height - 1)),
    }
}

/// Dibuja la letra sobre la franja ya oscurecida del lienzo: la línea actual se ilumina de
/// izquierda a derecha y la siguiente espera más tenue debajo.
pub fn draw_lyrics(buf: &mut Buffer, area: Rect, line: &LyricsLine) {
    let Some((top, _)) = lyrics_band(area.height, line.top) else { return };
    let width = area.width as usize;
    let room = width.saturating_sub(6);
    let center = |text: &str| width.saturating_sub(text.chars().count()) / 2;
    let current = text_fit(line.current, room);
    if !current.is_empty() {
        let x = center(&current);
        let total = current.chars().count();
        let lit = ((line.progress * total as f32).ceil() as usize).min(total);
        let sung: String = current.chars().take(lit).collect();
        let rest: String = current.chars().skip(lit).collect();
        let col = put(buf, area, line.mode, x, top + 1, &sung, line.pal.petal_light.mix(Rgb::WHITE, 0.75));
        put(buf, area, line.mode, col, top + 1, &rest, Rgb::WHITE.scale(0.50));
    }
    let next = text_fit(line.next, room);
    if !next.is_empty() {
        put(buf, area, line.mode, center(&next), top + 2, &next, Rgb::WHITE.scale(0.36));
    }
}

fn text_fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(width.saturating_sub(1)).collect::<String>())
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

    // Fila 1: título y artista a la izquierda; lo entendido, escena y volumen a la derecha.
    let track = session.track();
    let alert = session.warning().and_then(|w| w.short).map(|short| format!("{short} · d para detalles"));
    let (right, right_color) = match hud.notice.map(str::to_string).or(alert) {
        Some(warning) => (warning, pal.accent),
        None => {
            let scene = if hud.auto_scene { format!("{} (auto)", hud.scene.name()) } else { hud.scene.name().to_string() };
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
            text.push_str(&scene);
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
    let (mark, mark_color) = note_mark(hud.favorite, pal);
    put(buf, area, mode, 1, 0, mark, mark_color);
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
/// La nota antes del título: una estrella si la canción es favorita.
fn note_mark(favorite: bool, pal: &Palette) -> (&'static str, Rgb) {
    if favorite { ("★", pal.accent.mix(Rgb::WHITE, 0.35)) } else { ("♪", pal.petal.mix(Rgb::WHITE, 0.2)) }
}

fn draw_compact(buf: &mut Buffer, area: Rect, session: &dyn Session, pal: &Palette, mode: ColorMode, favorite: bool) {
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

    // Derecha: aviso corto, o tiempo, estado y volumen.
    let track = session.track();
    let mut right = String::new();
    if let Some(short) = session.warning().and_then(|w| w.short) {
        right.push_str(&format!("{short} · d"));
    } else {
        right.push_str(&clock(session.position()));
        if let Some(total) = track.duration {
            right.push_str(&format!(" / {}", clock(total)));
        }
        right.push_str(if session.playing() { "  ▶" } else { "  ⏸" });
        if let Some(v) = session.volume() {
            right.push_str(&format!("  vol {:.0}%", v * 100.0));
        }
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
    let (mark, mark_color) = note_mark(favorite, pal);
    put(1, mark, mark_color);
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
            let hud = Hud { session: &session, pal: &pal, mode: ColorMode::True, timeline: &timeline, bands: &bands, scene: SceneKind::Garden, auto_scene: false, favorite: false, notice: None };
            draw(&mut buf, area, &hud);
        }
    }

    /// Sesión de prueba con un aviso y una fuente de nombre largo.
    struct Noisy(crate::demo::DemoSession);

    impl Session for Noisy {
        fn tick(&mut self, dt: f32) -> crate::session::Tick {
            self.0.tick(dt)
        }
        fn track(&self) -> &crate::session::TrackInfo {
            self.0.track()
        }
        fn position(&self) -> Duration {
            self.0.position()
        }
        fn playing(&self) -> bool {
            true
        }
        fn finished(&self) -> bool {
            false
        }
        fn toggle_pause(&mut self) {}
        fn change_volume(&mut self, _: i32) {}
        fn label(&self) -> String {
            "Spotify · solo la app".into()
        }
        fn warning(&self) -> Option<crate::session::Warning> {
            Some(crate::session::Warning::new("sin señal", "No llega audio: revisa el permiso de grabación de audio del sistema."))
        }
    }

    fn text_of(buf: &Buffer) -> String {
        let area = buf.area;
        (0..area.height).flat_map(|y| (0..area.width).map(move |x| (x, y))).map(|(x, y)| buf[(x, y)].symbol().to_string()).collect()
    }

    #[test]
    fn player_shows_a_short_warning_and_no_capture_details() {
        let session = Noisy(crate::demo::DemoSession::new(60.0));
        let (pal, timeline, bands) = (Palette::default(), Timeline::new(), Bands::default());
        for rows in [3u16, 1] {
            let area = Rect::new(0, 0, 160, rows);
            let mut buf = Buffer::empty(area);
            let hud = Hud { session: &session, pal: &pal, mode: ColorMode::True, timeline: &timeline, bands: &bands, scene: SceneKind::Garden, auto_scene: false, favorite: false, notice: None };
            draw(&mut buf, area, &hud);
            let text = text_of(&buf);
            assert!(text.contains("sin señal"), "{rows} filas: {text}");
            assert!(!text.contains("solo la app"), "la fuente no va en el reproductor");
            assert!(!text.contains("permiso"), "el detalle no va en el reproductor");
        }

        // El panel de detalles sí lo muestra todo.
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);
        let song = crate::features::Song::default();
        let analysis = Analysis {
            song: &song,
            mood: Mood::Happy,
            origin: "auto",
            moment: Mood::Happy,
            source: session.label(),
            warning: session.warning().map(|w| w.detail),
            pal: &pal,
            mode: ColorMode::True,
        };
        draw_analysis(&mut buf, area, &analysis);
        let text = text_of(&buf);
        assert!(text.contains("solo la app") && text.contains("permiso"), "{text}");
    }

    #[test]
    fn wrap_breaks_on_words() {
        assert_eq!(wrap("uno dos tres cuatro", 8), ["uno dos", "tres", "cuatro"]);
        assert_eq!(wrap("", 8), Vec::<String>::new());
        assert_eq!(wrap("palabramuylarga", 5), ["palab"]);
    }

    #[test]
    fn clock_format() {
        assert_eq!(clock(Duration::from_secs(215)), "3:35");
    }

    #[test]
    fn card_fades_in_holds_and_fades_out() {
        assert_eq!(card_alpha(0.0), 0.0);
        assert!(card_alpha(2.0) > 0.99);
        assert_eq!(card_alpha(CARD_SECONDS), 0.0);
        assert!(card_alpha(0.3) < card_alpha(0.6));
        assert_eq!(card_band(5), None);
        let (a, b) = card_band(30).unwrap();
        assert!(a < b && b <= 30);
        assert_eq!(card_title("Hola", 40), "H O L A");
        assert_eq!(card_title("Una canción bastante larga", 30), "UNA CANCIÓN BASTANTE LARGA");
        assert!(card_title(&"x".repeat(80), 20).ends_with('…'));

        let pal = Palette::default();
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        draw_card(&mut buf, area, &Card { title: "Dissolve", artist: "OVERSIZE", alpha: 1.0, pal: &pal, mode: ColorMode::True });
        let text = text_of(&buf);
        assert!(text.contains("D I S S O L V E") && text.contains("OVERSIZE"), "{text}");
    }
}

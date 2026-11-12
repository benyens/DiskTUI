//! Menú de inicio: se abre al ejecutar `disktui` sin argumentos.
//!
//! Deja elegir fuente, escena y opciones sobre una vista previa animada. Por defecto apunta
//! a Spotify: Enter y empieza. Lo elegido se guarda en `~/.config/disktui/config` y vuelve
//! la próxima vez.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;

use crate::canvas::Canvas;
use crate::color::{ColorMode, Rgb};
use crate::cosmos::Cosmos;
use crate::demo;
use crate::city::City;
use crate::garden::Garden;
use crate::ocean::Ocean;
use crate::mood::Mood;
use crate::palette::Palette;
use crate::scene::{Frame, Scene, SceneChoice, SceneKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Spotify,
    /// Apple Music (experimental).
    Music,
    File,
    Demo,
}

/// Lo que se elige en el menú.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub source: Source,
    /// Ruta del archivo, para `Source::File`.
    pub file: String,
    pub scene: SceneChoice,
    pub flowers: u32,
    pub speed: f32,
    /// Spotify: capturar todo el sistema en vez de solo la app.
    pub global: bool,
    /// Clima fijo; `None` = automático, según la canción.
    pub mood: Option<Mood>,
    /// Mostrar la carátula del disco en una esquina.
    pub cover: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            source: Source::Spotify,
            file: String::new(),
            scene: SceneChoice::Auto,
            flowers: 3,
            speed: 1.0,
            global: false,
            mood: None,
            cover: false,
        }
    }
}

const SPEEDS: [f32; 5] = [0.5, 0.75, 1.0, 1.5, 2.0];

impl Settings {
    fn config_path() -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join(".config").join("disktui").join("config"))
    }

    /// Lee la configuración guardada. Si no existe o está dañada, usa los valores por defecto.
    pub fn load() -> Settings {
        Self::config_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|text| Settings::parse(&text))
            .unwrap_or_default()
    }

    /// Guarda la configuración. Si falla (carpeta sin permisos), no pasa nada: es comodidad.
    pub fn save(&self) {
        if let Some(path) = Self::config_path() {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, self.serialize());
        }
    }

    /// Formato `clave=valor`, una por línea. Lo que no se entiende se ignora.
    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let value = value.trim();
            match key.trim() {
                "fuente" => {
                    s.source = match value {
                        "archivo" => Source::File,
                        "demo" => Source::Demo,
                        "music" => Source::Music,
                        _ => Source::Spotify,
                    }
                }
                "archivo" => s.file = value.to_string(),
                "escena" => s.scene = SceneChoice::parse(value).unwrap_or(SceneChoice::Auto),
                "flores" => s.flowers = value.parse().unwrap_or(3).clamp(1, 3),
                "velocidad" => s.speed = value.parse().unwrap_or(1.0f32).clamp(0.25, 8.0),
                "captura" => s.global = value == "sistema",
                "animo" => s.mood = Mood::parse(value),
                "caratula" => s.cover = value == "si",
                _ => {}
            }
        }
        s
    }

    pub fn serialize(&self) -> String {
        format!(
            "fuente={}\narchivo={}\nescena={}\nflores={}\nvelocidad={}\ncaptura={}\nanimo={}\ncaratula={}\n",
            match self.source {
                Source::Spotify => "spotify",
                Source::Music => "music",
                Source::File => "archivo",
                Source::Demo => "demo",
            },
            self.file,
            self.scene.key(),
            self.flowers,
            self.speed,
            if self.global { "sistema" } else { "app" },
            self.mood.map(|m| m.name()).unwrap_or("auto"),
            if self.cover { "si" } else { "no" },
        )
    }
}

/// Limpia una ruta escrita o arrastrada a la terminal: quita comillas, deshace los escapes
/// con barra invertida (`Mi\ Cancion.mp3`) y expande `~/`.
pub fn clean_path(raw: &str) -> String {
    let trimmed = raw.trim();
    let quoted = trimmed.len() >= 2
        && ((trimmed.starts_with('\'') && trimmed.ends_with('\''))
            || (trimmed.starts_with('"') && trimmed.ends_with('"')));
    let mut out = String::new();
    if quoted {
        out.push_str(&trimmed[1..trimmed.len() - 1]);
    } else {
        let mut chars = trimmed.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            } else {
                out.push(c);
            }
        }
    }
    if let (Some(rest), Some(home)) = (out.strip_prefix("~/"), std::env::var_os("HOME")) {
        return PathBuf::from(home).join(rest).to_string_lossy().into_owned();
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Row {
    Source,
    File,
    Scene,
    Flowers,
    Mood,
    Speed,
    Cover,
    Capture,
}

/// Filas visibles según lo elegido: la ruta solo con archivo, la captura solo con Spotify.
fn rows(s: &Settings) -> Vec<Row> {
    let mut rows = vec![Row::Source];
    if s.source == Source::File {
        rows.push(Row::File);
    }
    rows.push(Row::Scene);
    if matches!(s.scene, SceneChoice::Auto | SceneChoice::Fixed(SceneKind::Garden)) {
        rows.push(Row::Flowers);
    }
    rows.push(Row::Mood);
    rows.push(Row::Speed);
    if s.source != Source::Demo {
        rows.push(Row::Cover);
    }
    if matches!(s.source, Source::Spotify | Source::Music) {
        rows.push(Row::Capture);
    }
    rows
}

/// Cambia el valor de una fila. `step` = −1 (←) o +1 (→).
fn change(s: &mut Settings, row: Row, step: i32) {
    let cycle = |i: usize, n: usize| ((i as i32 + step).rem_euclid(n as i32)) as usize;
    match row {
        Row::Source => {
            let order = [Source::Spotify, Source::Music, Source::File, Source::Demo];
            let i = order.iter().position(|x| *x == s.source).unwrap_or(0);
            s.source = order[cycle(i, order.len())];
        }
        Row::File => {}
        Row::Scene => s.scene = s.scene.next(),
        Row::Flowers => s.flowers = (cycle(s.flowers as usize - 1, 3) + 1) as u32,
        Row::Speed => {
            let i = SPEEDS
                .iter()
                .position(|v| (*v - s.speed).abs() < 0.01)
                .unwrap_or(2);
            s.speed = SPEEDS[(i as i32 + step).clamp(0, SPEEDS.len() as i32 - 1) as usize];
        }
        Row::Capture => s.global = !s.global,
        Row::Cover => s.cover = !s.cover,
        Row::Mood => {
            // Automático primero, luego los cinco climas.
            let i = s.mood.and_then(|m| Mood::ALL.iter().position(|x| *x == m)).map_or(0, |i| i + 1);
            let next = cycle(i, Mood::ALL.len() + 1);
            s.mood = if next == 0 { None } else { Some(Mood::ALL[next - 1]) };
        }
    }
}

fn value_text(s: &Settings, row: Row) -> String {
    match row {
        Row::Source => match s.source {
            Source::Spotify => "Spotify".into(),
            Source::Music => "Apple Music (experimental)".into(),
            Source::File => "Archivo".into(),
            Source::Demo => "Demo (sin audio)".into(),
        },
        Row::File => s.file.clone(),
        Row::Scene => s.scene.name().into(),
        Row::Cover => if s.cover { "visible" } else { "oculta" }.into(),
        Row::Flowers => s.flowers.to_string(),
        Row::Speed => format!("{}×", s.speed),
        Row::Capture => if s.global { "todo el sistema" } else { "solo la app" }.into(),
        Row::Mood => match s.mood {
            None => "según la canción".into(),
            Some(Mood::Happy) => "alegre · sol".into(),
            Some(Mood::Serene) => "serena · atardecer".into(),
            Some(Mood::Nostalgic) => "nostálgica · noche".into(),
            Some(Mood::Sad) => "triste · lluvia".into(),
            Some(Mood::Intense) => "intensa · tormenta".into(),
        },
    }
}

fn label(row: Row) -> &'static str {
    match row {
        Row::Source => "Fuente",
        Row::File => "Archivo",
        Row::Scene => "Escena",
        Row::Flowers => "Flores",
        Row::Speed => "Velocidad",
        Row::Capture => "Captura",
        Row::Cover => "Carátula",
        Row::Mood => "Clima",
    }
}

pub enum MenuResult {
    Start,
    Quit,
    /// Abrir el herbario y volver al menú.
    Herbarium,
}

/// Muestra el menú hasta que el usuario empieza o sale. Modifica `settings` en el lugar.
/// `error`: mensaje del intento anterior (archivo que no existe, captura que falló).
pub fn run(
    terminal: &mut DefaultTerminal,
    settings: &mut Settings,
    mode: ColorMode,
    mut error: Option<String>,
) -> Result<MenuResult> {
    let palette = Palette::default();
    let mut selected = 0usize;
    // Vista previa: las dos escenas corren con la señal del demo.
    let mut garden = Garden::new(0xD3E0, settings.flowers, None);
    garden.scenery.force_mood(settings.mood);
    let mut cosmos = Cosmos::new(11);
    let mut ocean = Ocean::new(0xD3E0);
    ocean.scenery.force_mood(settings.mood);
    let mut city = City::new(0xD3E0);
    city.scenery.force_mood(settings.mood);
    cosmos.force_mood(settings.mood);
    let mut canvas = Canvas::new(1, 1);
    let (mut clock, mut last, mut next_frame) = (18.0f32, Instant::now(), Instant::now());
    const PREVIEW_LEN: f32 = 75.0;

    loop {
        let visible = rows(settings);
        selected = selected.min(visible.len() - 1);
        let row = visible[selected];

        while event::poll(Duration::ZERO)? {
            match event::read()? {
                // Arrastrar un archivo a la terminal lo pega como ruta.
                Event::Paste(text) => {
                    settings.source = Source::File;
                    settings.file = clean_path(&text);
                    error = None;
                }
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let editing = row == Row::File;
                    match key.code {
                        KeyCode::Esc => return Ok(MenuResult::Quit),
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(MenuResult::Quit);
                        }
                        KeyCode::Enter => {
                            settings.file = clean_path(&settings.file);
                            return Ok(MenuResult::Start);
                        }
                        KeyCode::Up | KeyCode::BackTab => selected = selected.checked_sub(1).unwrap_or(visible.len() - 1),
                        KeyCode::Down | KeyCode::Tab => selected = (selected + 1) % visible.len(),
                        KeyCode::Backspace if editing => {
                            settings.file.pop();
                        }
                        KeyCode::Char(c) if editing => {
                            settings.file.push(c);
                            error = None;
                        }
                        KeyCode::Char('q') => return Ok(MenuResult::Quit),
                        KeyCode::Char('g') => return Ok(MenuResult::Herbarium),
                        KeyCode::Left => change(settings, row, -1),
                        KeyCode::Right | KeyCode::Char(' ') => change(settings, row, 1),
                        _ => {}
                    }
                    if garden.members.len() != settings.flowers as usize {
                        garden = Garden::new(0xD3E0, settings.flowers, None);
                    }
                    // La vista previa muestra el clima elegido.
                    garden.scenery.force_mood(settings.mood);
                }
                _ => {}
            }
        }

        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f32().clamp(0.0005, 0.1);
        last = now;
        clock += dt;
        if clock >= PREVIEW_LEN {
            clock = 0.0;
            garden.new_track();
            cosmos.new_track();
            ocean.new_track();
            city.new_track();
        }
        let mut bands = demo::bands_at(clock, PREVIEW_LEN, dt);
        bands.song = demo::song_for(settings.mood.unwrap_or(Mood::Nostalgic));
        let size = terminal.size()?;
        canvas.resize(size.width as usize, size.height as usize);
        let frame = Frame { dt, bands: &bands, progress: Some(clock / PREVIEW_LEN), song_len: PREVIEW_LEN, playing: true };
        for scene in [&mut garden as &mut dyn Scene, &mut cosmos, &mut ocean, &mut city] {
            scene.update(&frame, canvas.w, canvas.h);
        }
        // En automático, la vista previa muestra la escena del clima elegido o del demo.
        let heard = Mood::nearest(bands.song.valence, bands.song.arousal);
        match settings.scene.resolve(settings.mood.unwrap_or(heard)) {
            SceneKind::Garden => garden.draw(&mut canvas, &palette),
            SceneKind::Cosmos => cosmos.draw(&mut canvas, &palette),
            SceneKind::Ocean => ocean.draw(&mut canvas, &palette),
            SceneKind::City => city.draw(&mut canvas, &palette),
        }

        terminal.draw(|f| {
            let area = f.area();
            canvas.render(f.buffer_mut(), area, mode);
            draw_panel(f.buffer_mut(), area, settings, &visible, selected, error.as_deref(), &palette, mode);
        })?;

        next_frame += Duration::from_millis(33);
        let now = Instant::now();
        if next_frame > now {
            std::thread::sleep(next_frame - now);
        } else {
            next_frame = now;
        }
    }
}

/// Aviso a pantalla completa mientras se abre la fuente (la captura tarda unos segundos).
pub fn draw_notice(terminal: &mut DefaultTerminal, text: &str, mode: ColorMode) -> Result<()> {
    let pal = Palette::default();
    terminal.draw(|f| {
        let area = f.area();
        let buf = f.buffer_mut();
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].set_char(' ').set_style(Style::new().bg(mode.color(pal.bg_top)));
            }
        }
        let w = Span::raw(text).width() as u16;
        let x = area.x + area.width.saturating_sub(w) / 2;
        put(buf, area, mode, x, area.y + area.height / 2, text, pal.petal_light);
    })?;
    Ok(())
}

fn put(buf: &mut Buffer, clip: Rect, mode: ColorMode, x: u16, y: u16, text: &str, fg: Rgb) -> u16 {
    let mut col = x;
    for ch in text.chars() {
        if col >= clip.right() || y >= clip.bottom() {
            break;
        }
        let cell = &mut buf[(col, y)];
        cell.set_char(ch);
        cell.fg = mode.color(fg);
        col += Span::raw(ch.to_string()).width().max(1) as u16;
    }
    col
}

/// Recorta por la izquierda: de una ruta larga importa el final.
fn tail(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return text.to_string();
    }
    let keep: String = chars[chars.len() - max.saturating_sub(1)..].iter().collect();
    format!("…{keep}")
}

#[allow(clippy::too_many_arguments)]
fn draw_panel(
    buf: &mut Buffer,
    area: Rect,
    settings: &Settings,
    visible: &[Row],
    selected: usize,
    error: Option<&str>,
    pal: &Palette,
    mode: ColorMode,
) {
    let width = 60u16.min(area.width.saturating_sub(2));
    let height = (visible.len() as u16 + 8 + error.is_some() as u16).min(area.height);
    if width < 30 || height < 6 {
        return;
    }
    let panel = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    let bg = pal.bg_top.scale(0.55);
    let border = pal.petal.mix(bg, 0.35);
    let dim = bg.mix(Rgb::WHITE, 0.45);
    let bright = bg.mix(Rgb::WHITE, 0.92);

    for y in panel.top()..panel.bottom() {
        for x in panel.left()..panel.right() {
            let edge_x = x == panel.left() || x == panel.right() - 1;
            let edge_y = y == panel.top() || y == panel.bottom() - 1;
            let ch = match (edge_x, edge_y, x == panel.left(), y == panel.top()) {
                (true, true, true, true) => '╭',
                (true, true, false, true) => '╮',
                (true, true, true, false) => '╰',
                (true, true, false, false) => '╯',
                (true, false, _, _) => '│',
                (false, true, _, _) => '─',
                _ => ' ',
            };
            buf[(x, y)].set_char(ch).set_style(Style::new().fg(mode.color(border)).bg(mode.color(bg)));
        }
    }
    let title = " ✿ DiskTUI ";
    let tx = panel.x + (panel.width - Span::raw(title).width() as u16) / 2;
    put(buf, panel, mode, tx, panel.y, title, pal.petal.mix(Rgb::WHITE, 0.25));

    let inner = Rect { x: panel.x + 3, y: panel.y + 2, width: panel.width - 6, height: panel.height - 3 };
    for (i, row) in visible.iter().enumerate() {
        let y = inner.y + i as u16;
        if y >= panel.bottom() - 1 {
            break;
        }
        let active = i == selected;
        put(buf, inner, mode, inner.x, y, if active { "▸" } else { " " }, pal.petal);
        put(buf, inner, mode, inner.x + 2, y, label(*row), if active { bright } else { dim });
        let room = inner.width.saturating_sub(18) as usize;
        let color = if active { pal.accent.mix(Rgb::WHITE, 0.25) } else { bright.mix(bg, 0.25) };
        if *row == Row::File {
            // Campo de texto: cursor al final y pista si está vacío.
            if settings.file.is_empty() {
                put(buf, inner, mode, inner.x + 14, y, "escribe la ruta o arrastra el archivo", dim);
            } else {
                let end = put(buf, inner, mode, inner.x + 14, y, &tail(&settings.file, room.max(8)), color);
                if active {
                    put(buf, inner, mode, end, y, "▏", bright);
                }
            }
        } else {
            let text = format!("‹ {} ›", value_text(settings, *row));
            put(buf, inner, mode, inner.x + 14, y, &text, color);
        }
    }

    let mut y = inner.y + visible.len() as u16 + 1;
    if let Some(error) = error {
        if y < panel.bottom() - 1 {
            let text = tail(error, inner.width as usize);
            put(buf, inner, mode, inner.x, y, &text, Rgb(1.0, 0.55, 0.45));
        }
        y += 1;
    }
    let hints = ["Enter empezar   ↑↓ elegir   ←→ cambiar   Esc salir", "g herbario   ·   durante la música: m vuelve aquí"];
    for hint in hints {
        if y < panel.bottom() - 1 {
            put(buf, inner, mode, inner.x, y, hint, dim);
        }
        y += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_spotify() {
        let s = Settings::default();
        assert_eq!(s.source, Source::Spotify);
        assert!(!s.global);
        assert_eq!(Settings::parse(""), s);
        assert_eq!(Settings::parse("basura\nfuente=???\nflores=99"), Settings { flowers: 3, ..s });
    }

    #[test]
    fn settings_roundtrip() {
        let s = Settings {
            source: Source::File,
            file: "/Users/yo/Música/Mi Canción = mejor.mp3".into(),
            scene: SceneChoice::Fixed(SceneKind::Ocean),
            flowers: 2,
            speed: 1.5,
            global: true,
            mood: Some(Mood::Sad),
            cover: true,
        };
        assert_eq!(Settings::parse(&s.serialize()), s);
    }

    #[test]
    fn cleans_dragged_paths() {
        assert_eq!(clean_path("/a/Mi\\ Cancion\\ \\(vivo\\).mp3 "), "/a/Mi Cancion (vivo).mp3");
        assert_eq!(clean_path("'/a/Mi Cancion.mp3'"), "/a/Mi Cancion.mp3");
        assert_eq!(clean_path("\"/a/b.flac\""), "/a/b.flac");
        assert_eq!(clean_path("/a/sin-espacios.ogg"), "/a/sin-espacios.ogg");
    }

    #[test]
    fn rows_follow_the_choices() {
        let mut s = Settings::default();
        assert_eq!(
            rows(&s),
            [Row::Source, Row::Scene, Row::Flowers, Row::Mood, Row::Speed, Row::Cover, Row::Capture]
        );
        s.source = Source::File;
        s.scene = SceneChoice::Fixed(SceneKind::Cosmos);
        assert_eq!(rows(&s), [Row::Source, Row::File, Row::Scene, Row::Mood, Row::Speed, Row::Cover]);
        // El océano tiene clima pero no flores; el demo no tiene carátula.
        s.source = Source::Demo;
        s.scene = SceneChoice::Fixed(SceneKind::Ocean);
        assert_eq!(rows(&s), [Row::Source, Row::Scene, Row::Mood, Row::Speed]);
    }

    #[test]
    fn arrows_cycle_and_clamp() {
        let mut s = Settings::default();
        change(&mut s, Row::Source, 1);
        assert_eq!(s.source, Source::Music);
        change(&mut s, Row::Source, 1);
        assert_eq!(s.source, Source::File);
        change(&mut s, Row::Source, -1);
        change(&mut s, Row::Source, -1);
        change(&mut s, Row::Source, -1);
        assert_eq!(s.source, Source::Demo);
        change(&mut s, Row::Flowers, 1);
        assert_eq!(s.flowers, 1);
        for _ in 0..10 {
            change(&mut s, Row::Speed, 1);
        }
        assert_eq!(s.speed, 2.0);
        for _ in 0..10 {
            change(&mut s, Row::Speed, -1);
        }
        assert_eq!(s.speed, 0.5);
    }

    #[test]
    fn panel_draws_at_any_size() {
        let (s, pal) = (Settings::default(), Palette::default());
        for (w, h) in [(1u16, 1u16), (20, 5), (31, 8), (80, 24), (200, 60)] {
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            draw_panel(&mut buf, area, &s, &rows(&s), 0, Some("un error largo ".repeat(10).as_str()), &pal, ColorMode::True);
        }
    }
}

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
use crate::demo;
use crate::garden::Garden;
use crate::mood::Mood;
use crate::palette::Palette;
use crate::playlist::Playlist;
use crate::scene::{Frame, SceneChoice, SceneKind};
use crate::stage::Stage;

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
    /// En automática: entre qué escenas se elige y cómo se turnan.
    pub playlist: Playlist,
    pub flowers: u32,
    pub speed: f32,
    /// Spotify: capturar todo el sistema en vez de solo la app.
    pub global: bool,
    /// Clima fijo; `None` = automático, según la canción.
    pub mood: Option<Mood>,
    /// Mostrar la carátula del disco en una esquina.
    pub cover: bool,
    /// Tarjeta con el título al empezar cada canción.
    pub card: bool,
    /// Transición entre canciones.
    pub transition: crate::transition::Choice,
    /// El cielo sigue la hora del reloj en vez del ánimo de la canción.
    pub clock: bool,
    /// Letras sincronizadas desde LRCLIB.
    pub lyrics: bool,
    /// Cuándo bajar los cuadros por segundo para gastar menos batería.
    pub energy: crate::power::Energy,
}

impl Source {
    /// Las fuentes del menú, en orden.
    pub fn available() -> &'static [Source] {
        &[Source::Spotify, Source::Music, Source::File, Source::Demo]
    }

    /// La que viene elegida si no hay configuración.
    pub fn preferred() -> Source {
        Source::Spotify
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            source: Source::preferred(),
            file: String::new(),
            scene: SceneChoice::Auto,
            playlist: Playlist::default(),
            flowers: 3,
            speed: 1.0,
            global: false,
            mood: None,
            cover: false,
            card: true,
            transition: crate::transition::Choice::Auto,
            clock: false,
            lyrics: false,
            energy: crate::power::Energy::Auto,
        }
    }
}

const SPEEDS: [f32; 5] = [0.5, 0.75, 1.0, 1.5, 2.0];

impl Settings {
    fn config_path() -> Option<PathBuf> {
        Some(crate::paths::config_dir()?.join("config"))
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
        let (mut list, mut order, mut climax) = (String::new(), String::new(), false);
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
                    };
                    if !Source::available().contains(&s.source) {
                        s.source = Source::preferred();
                    }
                }
                "archivo" => s.file = value.to_string(),
                "escena" => s.scene = SceneChoice::parse(value).unwrap_or(SceneChoice::Auto),
                "lista" => list = value.to_string(),
                "orden" => order = value.to_string(),
                "climax" => climax = value == "si",
                "flores" => s.flowers = value.parse().unwrap_or(3).clamp(1, 3),
                "velocidad" => s.speed = value.parse().unwrap_or(1.0f32).clamp(0.25, 8.0),
                "captura" => s.global = value == "sistema",
                "animo" => s.mood = Mood::parse(value),
                "caratula" => s.cover = value == "si",
                "tarjeta" => s.card = value != "no",
                "cielo" => s.clock = value == "reloj",
                "letras" => s.lyrics = value == "si",
                "energia" => s.energy = crate::power::Energy::parse(value).unwrap_or(crate::power::Energy::Auto),
                "transicion" => s.transition = crate::transition::Choice::parse(value).unwrap_or(crate::transition::Choice::Auto),
                _ => {}
            }
        }
        s.playlist = Playlist::parse(&list, &order);
        s.playlist.on_climax = climax;
        s
    }

    pub fn serialize(&self) -> String {
        format!(
            "fuente={}\narchivo={}\nescena={}\nlista={}\norden={}\nclimax={}\nflores={}\nvelocidad={}\ncaptura={}\nanimo={}\ncaratula={}\ntarjeta={}\ntransicion={}\ncielo={}\nletras={}\nenergia={}\n",
            match self.source {
                Source::Spotify => "spotify",
                Source::Music => "music",
                Source::File => "archivo",
                Source::Demo => "demo",
            },
            self.file,
            self.scene.key(),
            self.playlist.key(),
            self.playlist.order.key(),
            if self.playlist.on_climax { "si" } else { "no" },
            self.flowers,
            self.speed,
            if self.global { "sistema" } else { "app" },
            self.mood.map(|m| m.name()).unwrap_or("auto"),
            if self.cover { "si" } else { "no" },
            if self.card { "si" } else { "no" },
            self.transition.key(),
            if self.clock { "reloj" } else { "cancion" },
            if self.lyrics { "si" } else { "no" },
            self.energy.key(),
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
    } else if cfg!(windows) {
        // En Windows la barra invertida separa carpetas: no es un escape.
        out.push_str(trimmed);
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
    if let (Some(rest), Some(home)) = (out.strip_prefix("~/"), crate::paths::home()) {
        return home.join(rest).to_string_lossy().into_owned();
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Row {
    Source,
    File,
    Scene,
    Playlist,
    Flowers,
    Mood,
    Speed,
    Cover,
    Card,
    Transition,
    Clock,
    Lyrics,
    Energy,
    Capture,
}

/// Filas visibles según lo elegido: la ruta solo con archivo, la captura solo con Spotify.
fn rows(s: &Settings) -> Vec<Row> {
    let mut rows = vec![Row::Source];
    if s.source == Source::File {
        rows.push(Row::File);
    }
    rows.push(Row::Scene);
    if s.scene == SceneChoice::Auto {
        rows.push(Row::Playlist);
    }
    // Solo con el jardín elegido a mano: en automática la escena puede ser cualquiera, y
    // si sale el jardín usa la cantidad guardada.
    if s.scene == SceneChoice::Fixed(SceneKind::Garden) {
        rows.push(Row::Flowers);
    }
    rows.push(Row::Mood);
    rows.push(Row::Speed);
    if s.source != Source::Demo {
        rows.push(Row::Cover);
    }
    rows.push(Row::Card);
    rows.push(Row::Transition);
    rows.push(Row::Clock);
    if s.source != Source::Demo {
        rows.push(Row::Lyrics);
    }
    rows.push(Row::Energy);
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
            let order = Source::available();
            let i = order.iter().position(|x| *x == s.source).unwrap_or(0);
            s.source = order[cycle(i, order.len())];
        }
        Row::File => {}
        Row::Scene => s.scene = s.scene.step(step),
        Row::Playlist => s.playlist.order = s.playlist.order.step(step),
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
        Row::Card => s.card = !s.card,
        Row::Transition => s.transition = s.transition.step(step),
        Row::Clock => s.clock = !s.clock,
        Row::Lyrics => s.lyrics = !s.lyrics,
        Row::Energy => s.energy = s.energy.step(step),
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
        Row::Playlist => s.playlist.summary(),
        Row::Cover => if s.cover { "visible" } else { "oculta" }.into(),
        Row::Card => if s.card { "al empezar cada canción" } else { "apagada" }.into(),
        Row::Transition => s.transition.name().into(),
        Row::Clock => if s.clock { "la hora de tu reloj" } else { "según la canción" }.into(),
        Row::Lyrics => if s.lyrics { "sincronizadas (LRCLIB)" } else { "apagadas" }.into(),
        Row::Energy => s.energy.name().into(),
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
        Row::Playlist => "Lista",
        Row::Flowers => "Flores",
        Row::Speed => "Velocidad",
        Row::Capture => "Captura",
        Row::Cover => "Carátula",
        Row::Card => "Tarjeta",
        Row::Transition => "Transición",
        Row::Clock => "Cielo",
        Row::Lyrics => "Letras",
        Row::Energy => "Energía",
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
    let mut stage = Stage::new(0xD3E0, Garden::new(0xD3E0, settings.flowers, None));
    stage.force_mood(settings.mood);
    let mut canvas = Canvas::new(1, 1);
    let (mut clock, mut last, mut next_frame) = (18.0f32, Instant::now(), Instant::now());
    const PREVIEW_LEN: f32 = 75.0;
    // Editor de la lista de escenas: `Some(i)` con la casilla `i` elegida.
    let mut list_cursor: Option<usize> = None;
    // Vueltas de la vista previa: en automática, cada vuelta es una "canción".
    let mut turns = 0u32;
    let mut preview_scene: Option<SceneKind> = None;

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
                Event::Key(key) if key.kind == KeyEventKind::Press && list_cursor.is_some() => {
                    let n = SceneKind::ALL.len();
                    let cur = list_cursor.unwrap_or(0);
                    let half = n.div_ceil(2);
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(MenuResult::Quit),
                        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => {
                            // Armar la lista cuesta: se guarda al cerrar, aunque después no se empiece.
                            list_cursor = None;
                            settings.save();
                        }
                        KeyCode::Up => list_cursor = Some((cur + n - 1) % n),
                        KeyCode::Down | KeyCode::Tab => list_cursor = Some((cur + 1) % n),
                        KeyCode::Left | KeyCode::Right => list_cursor = Some((cur + half) % (half * 2)).map(|c| c.min(n - 1)),
                        KeyCode::Enter | KeyCode::Char(' ') => {
                            settings.playlist.toggle(SceneKind::ALL[cur]);
                        }
                        KeyCode::Char('a') => settings.playlist.toggle_all(SceneKind::ALL[cur]),
                        KeyCode::Char('o') => settings.playlist.order = settings.playlist.order.step(1),
                        KeyCode::Char('c') => settings.playlist.on_climax = !settings.playlist.on_climax,
                        _ => {}
                    }
                }
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let editing = row == Row::File;
                    match key.code {
                        KeyCode::Esc => return Ok(MenuResult::Quit),
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(MenuResult::Quit);
                        }
                        KeyCode::Enter if row == Row::Playlist => {
                            let first = settings.playlist.scenes()[0];
                            list_cursor = SceneKind::ALL.iter().position(|k| *k == first);
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
                    // La vista previa muestra el clima elegido.
                    stage.force_mood(settings.mood);
                    if stage.garden.members.len() != settings.flowers as usize {
                        stage.set_garden(Garden::new(0xD3E0, settings.flowers, None));
                    }
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
            turns += 1;
            preview_scene = None;
            stage.new_track();
        }
        let mut bands = demo::bands_at(clock, PREVIEW_LEN, dt);
        bands.song = demo::song_for(settings.mood.unwrap_or(Mood::Nostalgic));
        let size = terminal.size()?;
        canvas.resize(size.width as usize, size.height as usize);
        let frame = Frame { dt, bands: &bands, progress: Some(clock / PREVIEW_LEN), song_len: PREVIEW_LEN, playing: true };
        stage.update(&frame, canvas.w, canvas.h);
        // En automático, la vista previa elige como lo haría la lista, una vez por vuelta;
        // en el editor de la lista, muestra la escena de la casilla elegida.
        let heard = Mood::nearest(bands.song.valence, bands.song.arousal);
        let shown = match (list_cursor, settings.scene) {
            (Some(i), _) => SceneKind::ALL[i],
            (None, SceneChoice::Fixed(kind)) => kind,
            (None, SceneChoice::Auto) => {
                let fits = preview_scene.filter(|k| settings.playlist.contains(*k));
                let pick = fits.unwrap_or_else(|| settings.playlist.pick(settings.mood.unwrap_or(heard), turns as u64, preview_scene, turns));
                preview_scene = Some(pick);
                pick
            }
        };
        stage.draw(shown, &mut canvas, &palette);

        terminal.draw(|f| {
            let area = f.area();
            canvas.render(f.buffer_mut(), area, mode);
            if let Some(cur) = list_cursor {
                draw_list_editor(f.buffer_mut(), area, settings, cur, &palette, mode);
            } else {
                draw_panel(f.buffer_mut(), area, settings, &visible, selected, error.as_deref(), &palette, mode);
            }
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

/// Caja con borde redondeado y título, como la del menú. Devuelve el área de adentro.
fn draw_box(buf: &mut Buffer, area: Rect, width: u16, height: u16, title: &str, pal: &Palette, mode: ColorMode) -> Option<(Rect, Rgb)> {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height);
    if width < 30 || height < 6 {
        return None;
    }
    let panel = Rect { x: area.x + (area.width - width) / 2, y: area.y + (area.height - height) / 2, width, height };
    let bg = pal.bg_top.scale(0.55);
    let border = pal.petal.mix(bg, 0.35);
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
    let tx = panel.x + (panel.width.saturating_sub(Span::raw(title).width() as u16)) / 2;
    put(buf, panel, mode, tx, panel.y, title, pal.petal.mix(Rgb::WHITE, 0.25));
    Some((Rect { x: panel.x + 3, y: panel.y + 2, width: panel.width - 6, height: panel.height - 3 }, bg))
}

/// Editor de la lista: las escenas en dos columnas con su casilla. La vista previa de
/// atrás muestra la escena elegida.
fn draw_list_editor(buf: &mut Buffer, area: Rect, settings: &Settings, cursor: usize, pal: &Palette, mode: ColorMode) {
    let n = SceneKind::ALL.len();
    let half = n.div_ceil(2);
    let Some((inner, bg)) = draw_box(buf, area, 62, half as u16 + 9, " ✿ Escenas en automática ", pal, mode) else { return };
    let dim = bg.mix(Rgb::WHITE, 0.45);
    let bright = bg.mix(Rgb::WHITE, 0.92);
    let list = &settings.playlist;
    let climax = if list.on_climax { " · y en el clímax" } else { "" };
    let order = format!("Orden  ‹ {} ›{climax}", list.order.name());
    put(buf, inner, mode, inner.x, inner.y, &order, pal.accent.mix(Rgb::WHITE, 0.25));
    let count = format!("{} de {}", list.len(), n);
    let cx = inner.right().saturating_sub(Span::raw(count.as_str()).width() as u16);
    put(buf, inner, mode, cx, inner.y, &count, dim);
    let col_w = inner.width / 2;
    for (i, kind) in SceneKind::ALL.iter().enumerate() {
        let (col, row) = (i / half, i % half);
        let x = inner.x + col as u16 * col_w;
        let y = inner.y + 2 + row as u16;
        if y >= inner.bottom() {
            continue;
        }
        let active = i == cursor;
        let on = list.contains(*kind);
        put(buf, inner, mode, x, y, if active { "▸" } else { " " }, pal.petal);
        let mark = if on { "[✓]" } else { "[ ]" };
        let color = match (on, active) {
            (true, true) => pal.accent.mix(Rgb::WHITE, 0.4),
            (true, false) => bright,
            (false, true) => dim.mix(Rgb::WHITE, 0.3),
            (false, false) => dim.mix(bg, 0.3),
        };
        put(buf, inner, mode, x + 2, y, mark, if on { pal.petal.mix(Rgb::WHITE, 0.3) } else { dim });
        put(buf, inner, mode, x + 6, y, kind.name(), color);
    }
    let mut y = inner.y + 3 + half as u16;
    // Según el ánimo: avisa qué ánimos se quedan sin escena propia.
    if list.order == crate::playlist::Order::Mood && !list.is_all() {
        let missing: Vec<&str> = Mood::ALL
            .iter()
            .filter(|m| !SceneKind::for_mood_all(**m).iter().any(|k| list.contains(*k)))
            .map(|m| m.name())
            .collect();
        if !missing.is_empty() && y < inner.bottom() {
            let text = tail(&format!("sin escena propia: {} (usan cualquiera de la lista)", missing.join(", ")), inner.width as usize);
            put(buf, inner, mode, inner.x, y, &text, Rgb(1.0, 0.75, 0.5));
        }
    }
    y += 1;
    for hint in ["espacio marcar   ↑↓←→ moverse   a todas/una   o orden", "c cambiar también en el clímax   ·   Esc volver al menú"] {
        if y < inner.bottom() + 1 {
            put(buf, inner, mode, inner.x, y, hint, dim);
        }
        y += 1;
    }
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
    let first = if visible.get(selected) == Some(&Row::Playlist) {
        "Enter editar la lista   ↑↓ elegir   ←→ orden   Esc salir"
    } else {
        "Enter empezar   ↑↓ elegir   ←→ cambiar   Esc salir"
    };
    let hints = [first, "g herbario   ·   durante la música: m vuelve aquí"];
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
            playlist: {
                let mut p = Playlist::new(&[SceneKind::Coast, SceneKind::Storm], crate::playlist::Order::Shuffle);
                p.on_climax = true;
                p
            },
            flowers: 2,
            speed: 1.5,
            global: true,
            mood: Some(Mood::Sad),
            cover: true,
            card: false,
            transition: crate::transition::Choice::Fixed(crate::transition::Style::Wave),
            clock: true,
            lyrics: true,
            energy: crate::power::Energy::Saver,
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
        // En automática no hay fila de flores; con el jardín elegido, sí.
        assert_eq!(rows(&s), [Row::Source, Row::Scene, Row::Playlist, Row::Mood, Row::Speed, Row::Cover, Row::Card, Row::Transition, Row::Clock, Row::Lyrics, Row::Energy, Row::Capture]);
        s.scene = SceneChoice::Fixed(SceneKind::Garden);
        assert_eq!(
            rows(&s),
            [Row::Source, Row::Scene, Row::Flowers, Row::Mood, Row::Speed, Row::Cover, Row::Card, Row::Transition, Row::Clock, Row::Lyrics, Row::Energy, Row::Capture]
        );
        s.source = Source::File;
        s.scene = SceneChoice::Fixed(SceneKind::Cosmos);
        assert_eq!(rows(&s), [Row::Source, Row::File, Row::Scene, Row::Mood, Row::Speed, Row::Cover, Row::Card, Row::Transition, Row::Clock, Row::Lyrics, Row::Energy]);
        // El océano tiene clima pero no flores; el demo no tiene carátula.
        s.source = Source::Demo;
        s.scene = SceneChoice::Fixed(SceneKind::Ocean);
        assert_eq!(rows(&s), [Row::Source, Row::Scene, Row::Mood, Row::Speed, Row::Card, Row::Transition, Row::Clock, Row::Energy]);
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
            draw_list_editor(&mut buf, area, &s, 7, &pal, ColorMode::True);
            let few = Settings { playlist: Playlist::new(&[SceneKind::Storm], crate::playlist::Order::Mood), ..s.clone() };
            draw_list_editor(&mut buf, area, &few, 19, &pal, ColorMode::True);
        }
    }
}

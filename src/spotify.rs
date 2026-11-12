//! Modo Spotify: captura el audio que reproduce la app y lee título, artista, posición y
//! carátula por AppleScript. Sin Web API, sin OAuth, sin credenciales.
//!
//! Única llamada de red de todo el programa: descargar la carátula (con `curl`).

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};

use crate::analysis::{self, Bands};
use crate::capture::{self, Capture, CaptureOptions};
use crate::palette::{Cover, Palette};
use crate::session::{Session, Tick, TrackInfo, drain_bands};

/// App de música que se visualiza.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum App {
    Spotify,
    /// Apple Music (la app "Música" de macOS). Soporte experimental.
    Music,
}

impl App {
    /// Nombre con que AppleScript conoce a la app.
    pub fn script_name(self) -> &'static str {
        match self {
            App::Spotify => "Spotify",
            App::Music => "Music",
        }
    }

    pub fn display(self) -> &'static str {
        match self {
            App::Spotify => "Spotify",
            App::Music => "Apple Music",
        }
    }

    /// Prefijo del bundle id de la app y sus procesos auxiliares, para el tap de audio.
    pub fn bundle_prefix(self) -> &'static str {
        match self {
            App::Spotify => "com.spotify.client",
            App::Music => "com.apple.Music",
        }
    }
}

/// Consulta para la app pedida. La de Apple Music sale de la de Spotify: mismo esqueleto,
/// otro identificador, duración en segundos y sin URL de carátula (se pide aparte).
fn query_script(app: App) -> String {
    match app {
        App::Spotify => QUERY.to_string(),
        App::Music => QUERY
            .replace("\"Spotify\"", "\"Music\"")
            .replace("(id of current track) as text", "(database ID of current track) as text")
            .replace("set dtDuration to duration of current track", "set dtDuration to (duration of current track) * 1000")
            .replace("        try\n            set dtArt to (artwork url of current track) as text\n        end try\n", ""),
    }
}

/// Apple Music no da una URL: la carátula se escribe a un archivo temporal.
fn music_artwork_script(path: &str) -> String {
    format!(
        "tell application \"Music\"\nset dtArtData to raw data of artwork 1 of current track\nend tell\n\
         set dtArtFile to open for access (POSIX file \"{path}\") with write permission\n\
         set eof of dtArtFile to 0\nwrite dtArtData to dtArtFile\nclose access dtArtFile\nreturn \"ok\""
    )
}

/// Separador de campos en la respuesta del AppleScript. Un texto raro a propósito: no
/// depende de saltos de línea y no aparece en títulos.
const SEP: &str = "|~|";

/// Devuelve `ok|~|estado|~|id|~|título|~|artista|~|duración ms|~|posición s|~|carátula|~|volumen`.
/// No abre Spotify si está cerrado. `player state` va sin `try`: si falta el permiso de
/// Automatización, el error debe salir. El resto va con `try` para que un campo ausente
/// (un podcast sin carátula, un archivo local) no bote toda la consulta.
///
/// Las variables llevan prefijo `dt` a propósito. AppleScript reserva palabras cortas que
/// parecen nombres inocentes: `st`, `nd`, `rd` y `th` son los sufijos de `1st`, `2nd`...
/// Usar `st` como variable da error de sintaxis (-2741).
const QUERY: &str = r#"
if application "Spotify" is running then
    tell application "Spotify"
        set dtSep to "|~|"
        set dtState to "stopped"
        set dtPlayer to player state
        if dtPlayer is playing then
            set dtState to "playing"
        else if dtPlayer is paused then
            set dtState to "paused"
        end if
        set dtId to ""
        set dtName to ""
        set dtArtist to ""
        set dtDuration to 0
        set dtPosition to 0
        set dtArt to ""
        set dtVolume to -1
        try
            set dtId to (id of current track) as text
        end try
        try
            set dtName to (name of current track) as text
        end try
        try
            set dtArtist to (artist of current track) as text
        end try
        try
            set dtDuration to duration of current track
        end try
        try
            set dtPosition to player position
        end try
        try
            set dtArt to (artwork url of current track) as text
        end try
        try
            set dtVolume to sound volume
        end try
        return "ok" & dtSep & dtState & dtSep & dtId & dtSep & dtName & dtSep & dtArtist & dtSep & (dtDuration as text) & dtSep & (dtPosition as text) & dtSep & dtArt & dtSep & (dtVolume as text)
    end tell
else
    return "closed"
end if
"#;

#[derive(Clone, Debug, Default, PartialEq)]
struct NowPlaying {
    id: String,
    title: String,
    artist: String,
    duration: Option<Duration>,
    position: f32,
    playing: bool,
    artwork_url: String,
    volume: Option<f32>,
}

/// Interpreta la salida del AppleScript. `None` = Spotify cerrado o sin canción cargada.
fn parse(output: &str) -> Option<NowPlaying> {
    let fields: Vec<&str> = output.trim_end_matches(['\r', '\n']).split(SEP).collect();
    if fields.len() < 9 || fields[0] != "ok" {
        return None;
    }
    // AppleScript escribe los decimales según la región: "12,5" en Chile. Y los números
    // grandes pueden venir en notación científica: "2,15E+5".
    let number = |s: &str| s.trim().replace(',', ".").parse::<f32>().ok();
    let id = fields[2].trim().to_string();
    let title = fields[3].trim().to_string();
    if id.is_empty() && title.is_empty() {
        return None;
    }
    Some(NowPlaying {
        playing: fields[1].trim() == "playing",
        // Sin id (archivo local), el título y el artista identifican la canción.
        id: if id.is_empty() { format!("{title}|{}", fields[4].trim()) } else { id },
        title,
        artist: fields[4].trim().to_string(),
        duration: number(fields[5]).filter(|ms| *ms > 0.0).map(|ms| Duration::from_secs_f32(ms / 1000.0)),
        position: number(fields[6]).unwrap_or(0.0).max(0.0),
        artwork_url: fields[7].trim().to_string(),
        volume: number(fields[8]).filter(|v| *v >= 0.0).map(|v| (v / 100.0).clamp(0.0, 1.0)),
    })
}

/// Ejecuta la consulta. `Err` trae el mensaje de AppleScript (por ejemplo, permiso denegado).
fn query(app: App) -> Result<Option<NowPlaying>, String> {
    let out = Command::new("osascript")
        .arg("-e")
        .arg(query_script(app))
        .output()
        .map_err(|e| format!("no se pudo ejecutar osascript: {e}"))?;
    if !out.status.success() {
        return Err(clean_error(&String::from_utf8_lossy(&out.stderr)));
    }
    Ok(parse(&String::from_utf8_lossy(&out.stdout)))
}

/// Quita el prefijo de posición ("123:145: execution error: ") del error de osascript.
fn clean_error(stderr: &str) -> String {
    let line = stderr.lines().next().unwrap_or("").trim();
    let msg = line.split_once("error: ").map(|(_, m)| m).unwrap_or(line);
    if msg.is_empty() { "AppleScript falló sin mensaje".into() } else { msg.to_string() }
}

/// Estado de Spotify en texto legible, más la respuesta cruda. Para el diagnóstico.
pub fn describe_state(app: App) -> Result<String, String> {
    let out = Command::new("osascript")
        .arg("-e")
        .arg(query_script(app))
        .output()
        .map_err(|e| format!("no se pudo ejecutar osascript: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let raw = text.trim();
    Ok(match parse(&text) {
        Some(n) => format!(
            "{} · \"{}\" — {} · {:.0} s de {:.0} s · carátula: {}\n    crudo: {raw}",
            if n.playing { "reproduciendo" } else { "en pausa" },
            n.title,
            n.artist,
            n.position,
            n.duration.map(|d| d.as_secs_f32()).unwrap_or(0.0),
            if n.artwork_url.is_empty() { "sin URL" } else { "con URL" },
        ),
        None => format!("sin canción · crudo: {raw}"),
    })
}

/// Descarga la carátula de la canción actual y dice si se pudo leer. Para el diagnóstico.
pub fn describe_artwork(app: App) -> String {
    let Ok(Some(now)) = query(app) else { return "no hay canción".into() };
    match artwork(app, &now.artwork_url) {
        None => format!("no se pudo descargar {}", now.artwork_url),
        Some(bytes) => match Palette::from_image_bytes(&bytes) {
            Some(p) => {
                let (r, g, b) = p.petal.to_u8();
                format!("ok: {} bytes · color de pétalos #{r:02x}{g:02x}{b:02x}", bytes.len())
            }
            None => format!("descargada ({} bytes) pero no es JPEG ni PNG legible", bytes.len()),
        },
    }
}

fn osascript(script: &str) -> Option<String> {
    let out = Command::new("osascript").arg("-e").arg(script).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Bytes de la carátula de la canción actual.
fn artwork(app: App, url: &str) -> Option<Vec<u8>> {
    match app {
        App::Spotify => fetch_artwork(url),
        App::Music => {
            let path = format!("/tmp/disktui-art-{}", std::process::id());
            osascript(&music_artwork_script(&path))?;
            let bytes = std::fs::read(&path).ok();
            let _ = std::fs::remove_file(&path);
            bytes.filter(|b| !b.is_empty())
        }
    }
}

/// Descarga la carátula. Solo acepta URLs https.
fn fetch_artwork(url: &str) -> Option<Vec<u8>> {
    if !url.starts_with("https://") {
        return None;
    }
    let out = Command::new("curl")
        .args(["-sL", "--max-time", "6", "--proto", "=https", "--"])
        .arg(url)
        .output()
        .ok()?;
    (out.status.success() && !out.stdout.is_empty()).then_some(out.stdout)
}

enum Command_ {
    PlayPause,
    Volume(i32),
    /// +1 siguiente, −1 anterior.
    Skip(i32),
}

enum Update {
    State(Result<Option<NowPlaying>, String>),
    Palette(Palette, Option<Cover>),
}

/// Hilo que consulta a Spotify una vez por segundo y atiende los comandos del teclado.
fn spawn_poller(app: App, tx: Sender<Update>, commands: Receiver<Command_>, stop: Arc<AtomicBool>) {
    let name = app.script_name();
    let _ = thread::Builder::new().name("disktui-spotify".into()).spawn(move || {
        let mut current_id = String::new();
        while !stop.load(Ordering::Relaxed) {
            let now = query(app);
            let new_track = now.as_ref().ok().and_then(|n| n.as_ref()).filter(|n| n.id != current_id).cloned();
            if tx.send(Update::State(now)).is_err() {
                break;
            }
            if let Some(track) = new_track {
                current_id = track.id.clone();
                let bytes = artwork(app, &track.artwork_url);
                let palette = bytes.as_deref().and_then(Palette::from_image_bytes).unwrap_or_default();
                let cover = bytes.as_deref().and_then(Cover::from_image_bytes);
                if tx.send(Update::Palette(palette, cover)).is_err() {
                    break;
                }
            }
            // Espera 1 s, o menos si llega un comando: tras ejecutarlo se vuelve a consultar.
            match commands.recv_timeout(Duration::from_secs(1)) {
                Ok(Command_::PlayPause) => {
                    osascript(&format!("tell application \"{name}\" to playpause"));
                }
                Ok(Command_::Volume(steps)) => {
                    osascript(&format!(
                        "tell application \"{name}\"\nset dtVolume to (sound volume) + ({})\nif dtVolume > 100 then set dtVolume to 100\nif dtVolume < 0 then set dtVolume to 0\nset sound volume to dtVolume\nend tell",
                        steps * 5
                    ));
                }
                Ok(Command_::Skip(direction)) => {
                    let verb = if direction > 0 { "next track" } else { "previous track" };
                    osascript(&format!("tell application \"{name}\" to {verb}"));
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

pub struct SpotifySession {
    app: App,
    capture: Capture,
    rx: Receiver<Bands>,
    last: Bands,
    stop: Arc<AtomicBool>,
    updates: Receiver<Update>,
    commands: Sender<Command_>,
    now: Option<NowPlaying>,
    /// Momento en que llegó `now`, para interpolar la posición entre consultas.
    now_at: Instant,
    info: TrackInfo,
    current_id: String,
    /// Segundos seguidos reproduciendo sin que llegue señal.
    silent_for: f32,
    started: Instant,
}

impl SpotifySession {
    pub fn open(app: App, device: Option<String>, global: bool) -> Result<Self> {
        let options = CaptureOptions { device, global, bundle_prefix: app.bundle_prefix() };
        let mut capture = capture::open(&options)?;
        let consumer = capture.consumer.take().ok_or_else(|| anyhow!("captura sin buffer"))?;

        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        analysis::spawn(consumer, capture.link.clone(), tx, stop.clone());

        let (update_tx, updates) = mpsc::channel();
        let (commands, command_rx) = mpsc::channel();
        spawn_poller(app, update_tx, command_rx, stop.clone());

        Ok(SpotifySession {
            app,
            capture,
            rx,
            last: Bands::default(),
            stop,
            updates,
            commands,
            now: None,
            now_at: Instant::now(),
            info: TrackInfo { title: format!("Esperando a {}…", app.display()), ..TrackInfo::default() },
            current_id: String::new(),
            silent_for: 0.0,
            started: Instant::now(),
        })
    }
}

impl Session for SpotifySession {
    fn tick(&mut self, dt: f32) -> Tick {
        let mut tick = Tick { bands: drain_bands(&self.rx, &mut self.last), ..Tick::default() };
        while let Ok(update) = self.updates.try_recv() {
            match update {
                Update::Palette(p, cover) => {
                    tick.palette = Some(p);
                    tick.cover = cover;
                }
                Update::State(state) => {
                    self.now_at = Instant::now();
                    match &state {
                        Ok(Some(n)) => {
                            if n.id != self.current_id {
                                // La primera canción no marchita nada: aún no hay planta.
                                tick.new_track = !self.current_id.is_empty();
                                self.current_id = n.id.clone();
                                // El análisis olvida el tempo y la tonalidad de la anterior.
                                self.capture.link.reset.store(true, Ordering::Relaxed);
                            }
                            self.info = TrackInfo {
                                title: n.title.clone(),
                                artist: n.artist.clone(),
                                duration: n.duration,
                                known: true,
                            };
                        }
                        Ok(None) => {
                            self.info = TrackInfo {
                                title: format!("{} cerrado o sin canción", self.app.display()),
                                ..TrackInfo::default()
                            };
                        }
                        // El error real a la vista: casi siempre es el permiso de Automatización.
                        Err(e) => {
                            self.info = TrackInfo { title: format!("AppleScript: {e}"), ..TrackInfo::default() };
                        }
                    }
                    self.now = state.ok().flatten();
                }
            }
        }
        if self.playing() && tick.bands.level < 0.01 {
            self.silent_for += dt;
        } else {
            self.silent_for = 0.0;
        }
        tick
    }

    fn track(&self) -> &TrackInfo {
        &self.info
    }

    fn position(&self) -> Duration {
        let Some(n) = &self.now else { return Duration::ZERO };
        let mut pos = n.position;
        if n.playing {
            pos += self.now_at.elapsed().as_secs_f32();
        }
        if let Some(total) = n.duration {
            pos = pos.min(total.as_secs_f32());
        }
        Duration::from_secs_f32(pos)
    }

    fn playing(&self) -> bool {
        match &self.now {
            Some(n) => n.playing,
            // Sin datos de Spotify, manda el audio: si llega señal, la escena sigue creciendo.
            None => self.last.level > 0.02,
        }
    }

    fn toggle_pause(&mut self) {
        let _ = self.commands.send(Command_::PlayPause);
    }

    fn change_volume(&mut self, steps: i32) {
        let _ = self.commands.send(Command_::Volume(steps));
    }

    fn skip(&mut self, direction: i32) {
        let _ = self.commands.send(Command_::Skip(direction));
    }

    fn volume(&self) -> Option<f32> {
        self.now.as_ref().and_then(|n| n.volume)
    }

    fn label(&self) -> String {
        format!("{} · {}", self.app.display(), self.capture.label)
    }

    fn warning(&self) -> Option<String> {
        // Con Spotify en volumen 0 el silencio es esperable: el tap captura después del
        // control de volumen de la app. No es un problema de permisos.
        let muted = self.volume().is_some_and(|v| v < 0.03);
        if self.silent_for > 4.0 && muted {
            Some(format!("{} está en volumen 0: sube con +", self.app.display()))
        } else if self.silent_for > 4.0 {
            Some("sin señal: revisa el permiso de audio del sistema (README)".into())
        } else if self.started.elapsed() < Duration::from_secs(8) {
            // El aviso de arranque se muestra unos segundos y luego deja ver el tiempo.
            self.capture.note.clone()
        } else {
            None
        }
    }
}

impl Drop for SpotifySession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_applescript_output_with_comma_decimals() {
        let out = "ok|~|playing|~|spotify:track:abc|~|Canción|~|Artista|~|215000|~|12,5|~|https://i.scdn.co/image/x|~|80\n";
        let n = parse(out).unwrap();
        assert!(n.playing);
        assert_eq!(n.title, "Canción");
        assert_eq!(n.duration, Some(Duration::from_secs(215)));
        assert!((n.position - 12.5).abs() < 1e-4);
        assert_eq!(n.volume, Some(0.8));
    }

    #[test]
    fn parses_scientific_notation_and_missing_fields() {
        // Duración en notación científica, sin carátula ni volumen (los `try` fallaron).
        let n = parse("ok|~|paused|~||~|Tema local|~|Alguien|~|2,15E+5|~|0|~||~|-1").unwrap();
        assert!(!n.playing);
        assert_eq!(n.duration, Some(Duration::from_secs(215)));
        assert_eq!(n.volume, None);
        assert_eq!(n.id, "Tema local|Alguien");
    }

    #[test]
    fn closed_or_empty_is_none() {
        assert!(parse("closed\n").is_none());
        assert!(parse("ok|~|stopped|~||~||~||~|0|~|0|~||~|50").is_none());
    }

    #[test]
    fn script_variables_are_not_reserved_words() {
        // Toda variable asignada con `set X to` debe llevar el prefijo `dt`, salvo la
        // propiedad `sound volume` de Spotify.
        let scripts = [query_script(App::Spotify), query_script(App::Music), music_artwork_script("/tmp/x")];
        for script in &scripts {
            for line in script.lines().map(str::trim).filter(|l| l.starts_with("set ")) {
                let name = line.trim_start_matches("set ").split(" to ").next().unwrap();
                let allowed = name.starts_with("dt") || name == "sound volume" || name == "eof of dtArtFile";
                assert!(allowed, "variable sin prefijo: {name}");
            }
            // Y los sufijos ordinales no aparecen sueltos en ninguna parte.
            for word in script.split(|c: char| !c.is_alphanumeric()) {
                assert!(!["st", "nd", "rd", "th"].contains(&word), "palabra reservada: {word}");
            }
        }
    }

    #[test]
    fn apple_music_script_differs_where_it_must() {
        let music = query_script(App::Music);
        assert!(music.contains("tell application \"Music\"") && !music.contains("Spotify"));
        assert!(music.contains("database ID of current track"));
        assert!(music.contains("(duration of current track) * 1000"));
        // Music no tiene `artwork url`: dejarlo sería un error de sintaxis.
        assert!(!music.contains("artwork url"));
        // El de Spotify queda intacto.
        assert!(query_script(App::Spotify).contains("artwork url of current track"));
    }

    #[test]
    fn cleans_osascript_errors() {
        let raw = "34:46: execution error: No autorizado para enviar eventos Apple a Spotify. (-1743)\n";
        assert_eq!(clean_error(raw), "No autorizado para enviar eventos Apple a Spotify. (-1743)");
    }

    #[test]
    fn artwork_must_be_https() {
        assert!(fetch_artwork("file:///etc/passwd").is_none());
        assert!(fetch_artwork("http://example.com/a.jpg").is_none());
    }
}

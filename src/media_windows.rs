//! Windows: qué está sonando, a través de los controles multimedia del sistema (GSMTC).
//!
//! Son los mismos datos que muestra Windows en el panel de volumen: título, artista,
//! carátula, posición y estado, más play/pausa, siguiente, anterior y salto. Funciona con
//! Spotify, Apple Music para Windows y cualquier app que se registre ahí.

use std::cell::RefCell;
use std::time::{SystemTime, UNIX_EPOCH};

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as MediaSession,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::DataReader;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};

/// Lo que suena en la app.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Media {
    pub title: String,
    pub artist: String,
    /// Duración y posición en segundos.
    pub duration: f32,
    pub position: f32,
    pub playing: bool,
}

/// Una orden para la app.
#[derive(Clone, Copy, Debug)]
pub enum Control {
    PlayPause,
    Next,
    Previous,
    /// Segundos a adelantar (negativo: retroceder).
    Seek(f32),
}

thread_local! {
    /// El administrador de sesiones se pide una vez por hilo: pedirlo cuesta.
    static MANAGER: RefCell<Option<Manager>> = const { RefCell::new(None) };
}

/// Unidades de 100 ns por segundo, la escala de los tiempos de WinRT.
const TICKS: f64 = 10_000_000.0;
/// Diferencia entre 1601-01-01 (época de WinRT) y 1970-01-01, en unidades de 100 ns.
const EPOCH_1601: i64 = 116_444_736_000_000_000;

fn manager() -> Result<Manager, String> {
    MANAGER.with(|cell| {
        if let Some(m) = cell.borrow().as_ref() {
            return Ok(m.clone());
        }
        // Las llamadas WinRT necesitan el hilo inicializado. Si ya lo estaba, no pasa nada.
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        let m = Manager::RequestAsync()
            .and_then(|op| op.join())
            .map_err(|e| format!("Windows no entregó los controles multimedia: {e}"))?;
        *cell.borrow_mut() = Some(m.clone());
        Ok(m)
    })
}

/// La sesión de la app cuyo identificador contiene `app` (sin distinguir mayúsculas). Sin
/// coincidencia, `None`: no se muestra lo que suene en otra app.
fn session(app: &str) -> Result<Option<MediaSession>, String> {
    let manager = manager()?;
    let sessions = manager.GetSessions().map_err(|e| e.to_string())?;
    let wanted = app.to_lowercase();
    for s in sessions {
        let id = s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
        if id.to_lowercase().contains(&wanted) {
            return Ok(Some(s));
        }
    }
    Ok(None)
}

/// Ahora, en la escala de WinRT.
fn now_ticks() -> i64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    EPOCH_1601 + (since.as_nanos() / 100) as i64
}

/// Lo que suena en la app. `Ok(None)`: la app no está abierta o no tiene canción.
pub fn query(app: &str) -> Result<Option<Media>, String> {
    let Some(session) = session(app)? else { return Ok(None) };
    let props = session
        .TryGetMediaPropertiesAsync()
        .and_then(|op| op.join())
        .map_err(|e| format!("no se pudo leer la canción: {e}"))?;
    let title = props.Title().map(|h| h.to_string()).unwrap_or_default();
    let artist = props.Artist().map(|h| h.to_string()).unwrap_or_default();
    if title.is_empty() && artist.is_empty() {
        return Ok(None);
    }
    let playing = session
        .GetPlaybackInfo()
        .and_then(|info| info.PlaybackStatus())
        .is_ok_and(|status| status == Status::Playing);
    let (mut position, mut duration) = (0.0f32, 0.0f32);
    if let Ok(timeline) = session.GetTimelineProperties() {
        let start = timeline.StartTime().map(|t| t.Duration).unwrap_or(0);
        let end = timeline.EndTime().map(|t| t.Duration).unwrap_or(0);
        let pos = timeline.Position().map(|t| t.Duration).unwrap_or(0);
        duration = ((end - start) as f64 / TICKS) as f32;
        position = ((pos - start) as f64 / TICKS) as f32;
        // La posición es la del último aviso de la app: si sigue sonando, ha avanzado.
        if playing && let Ok(updated) = timeline.LastUpdatedTime() {
            let age = ((now_ticks() - updated.UniversalTime) as f64 / TICKS) as f32;
            if (0.0..30.0).contains(&age) {
                position += age;
            }
        }
        if duration > 0.0 {
            position = position.clamp(0.0, duration);
        }
    }
    Ok(Some(Media { title, artist, duration: duration.max(0.0), position: position.max(0.0), playing }))
}

/// Bytes de la carátula de la canción actual, si la app la publicó.
pub fn thumbnail(app: &str) -> Option<Vec<u8>> {
    let session = session(app).ok()??;
    let props = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
    let stream = props.Thumbnail().ok()?.OpenReadAsync().ok()?.join().ok()?;
    let size = stream.Size().ok()? as u32;
    if size == 0 {
        return None;
    }
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    let loaded = reader.LoadAsync(size).ok()?.join().ok()?;
    let mut bytes = vec![0u8; loaded as usize];
    reader.ReadBytes(&mut bytes).ok()?;
    (!bytes.is_empty()).then_some(bytes)
}

/// Le da una orden a la app. Si no se puede (la app no lo permite), no hace nada.
pub fn control(app: &str, control: Control) {
    let Ok(Some(session)) = session(app) else { return };
    let _ = match control {
        Control::PlayPause => session.TryTogglePlayPauseAsync().and_then(|op| op.join()),
        Control::Next => session.TrySkipNextAsync().and_then(|op| op.join()),
        Control::Previous => session.TrySkipPreviousAsync().and_then(|op| op.join()),
        Control::Seek(seconds) => {
            let Ok(timeline) = session.GetTimelineProperties() else { return };
            let start = timeline.StartTime().map(|t| t.Duration).unwrap_or(0);
            let end = timeline.EndTime().map(|t| t.Duration).unwrap_or(i64::MAX);
            let pos = timeline.Position().map(|t| t.Duration).unwrap_or(0);
            let target = (pos + (seconds as f64 * TICKS) as i64).clamp(start, end.max(start));
            session.TryChangePlaybackPositionAsync(target).and_then(|op| op.join())
        }
    };
}

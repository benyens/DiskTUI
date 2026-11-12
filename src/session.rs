//! Interfaz común de las fuentes de audio: archivo, Spotify y demo.

use std::sync::mpsc::Receiver;
use std::time::Duration;

use crate::analysis::Bands;
use crate::palette::{Cover, Palette};
use crate::structure::SongMap;

#[derive(Clone, Debug, Default)]
pub struct TrackInfo {
    pub title: String,
    pub artist: String,
    pub duration: Option<Duration>,
    /// Es una canción real con datos fiables (no el demo ni un aviso): puede ir al herbario.
    pub known: bool,
}

/// Lo que una fuente entrega en cada cuadro.
#[derive(Default)]
pub struct Tick {
    pub bands: Bands,
    /// Cambió la canción: la planta se marchita y nace otra.
    pub new_track: bool,
    /// Paleta nueva (carátula recién leída).
    pub palette: Option<Palette>,
    /// Miniatura de la carátula; llega junto con la paleta (`None` si no hay imagen).
    pub cover: Option<Cover>,
}

pub trait Session {
    /// Se llama una vez por cuadro.
    fn tick(&mut self, dt: f32) -> Tick;
    fn track(&self) -> &TrackInfo;
    fn position(&self) -> Duration;
    fn playing(&self) -> bool;
    /// La fuente terminó (fin del archivo).
    fn finished(&self) -> bool {
        false
    }
    fn toggle_pause(&mut self);
    /// Salta a la canción siguiente (+1) o anterior (−1), si la fuente lo permite.
    fn skip(&mut self, _direction: i32) {}
    /// `steps` positivo sube el volumen, negativo lo baja.
    fn change_volume(&mut self, steps: i32);
    /// Volumen 0..1, si la fuente lo conoce.
    fn volume(&self) -> Option<f32> {
        None
    }
    /// Etiqueta corta para el HUD: de dónde viene el audio.
    fn label(&self) -> String;
    /// Mapa de la canción completa, cuando se conoce (modo archivo).
    fn song_map(&self) -> Option<&SongMap> {
        None
    }
    /// Aviso para el HUD (por ejemplo, captura sin señal).
    fn warning(&self) -> Option<String> {
        None
    }

    /// Avance de la canción en 0..1, si se conoce la duración.
    fn progress(&self) -> Option<f32> {
        let total = self.track().duration?.as_secs_f32();
        (total > 0.0).then(|| (self.position().as_secs_f32() / total).clamp(0.0, 1.0))
    }
}

/// Vacía el canal del hilo de análisis y devuelve el dato más reciente.
/// Los eventos (`kick`, `hat`, `climax`, `section`, `beat`) se acumulan para no perder ninguno entre cuadros.
pub fn drain_bands(rx: &Receiver<Bands>, last: &mut Bands) -> Bands {
    let (mut kick, mut hat, mut climax, mut section, mut beat) = (false, false, false, false, false);
    while let Ok(b) = rx.try_recv() {
        kick |= b.kick;
        hat |= b.hat;
        climax |= b.climax;
        section |= b.section;
        beat |= b.beat;
        *last = b;
    }
    Bands { kick, hat, climax, section, beat, ..*last }
}

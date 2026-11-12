//! Modo demo: bandas sintéticas, sin audio. Sirve para ver la animación sin un archivo.

use std::f32::consts::TAU;
use std::time::Duration;

use crate::analysis::Bands;
use crate::features::Song;
use crate::mood::Mood;
use crate::session::{Session, Tick, TrackInfo};

/// Ritmo sintético a 112 BPM con intro, subida, clímax y cierre.
pub fn bands_at(t: f32, len: f32, dt: f32) -> Bands {
    let p = (t / len).clamp(0.0, 1.0);
    // Intensidad por sección de la "canción".
    let energy = if p < 0.12 {
        0.35 + 1.5 * p
    } else if p < 0.40 {
        0.55 + 0.9 * (p - 0.12)
    } else if p < 0.72 {
        1.0
    } else if p < 0.80 {
        0.45
    } else {
        0.9 - 1.2 * (p - 0.80)
    };

    let beat = t * 112.0 / 60.0;
    let phase = beat.fract();
    let half = (beat * 2.0).fract();
    let crossed = |x: f32, rate: f32| x < dt * rate;

    let kick_env = (-phase * 5.0).exp();
    let hat_env = (-half * 9.0).exp() * if (beat * 2.0) as u32 % 2 == 1 { 1.0 } else { 0.45 };
    let melody = 0.5 + 0.5 * (t * 0.9).sin() * (t * 0.37).cos();

    Bands {
        bass: (energy * (0.18 + 0.82 * kick_env)).clamp(0.0, 1.0),
        mid: (energy * (0.25 + 0.55 * melody + 0.1 * (t * TAU * 0.5).sin())).clamp(0.0, 1.0),
        treble: (energy * energy * (0.10 + 0.85 * hat_env)).clamp(0.0, 1.0),
        level: (energy * (0.45 + 0.35 * kick_env + 0.1 * melody)).clamp(0.0, 1.0),
        kick: crossed(phase, 112.0 / 60.0),
        hat: energy > 0.5 && crossed(half, 224.0 / 60.0),
        intensity: ((energy - 0.35) / 0.65).clamp(0.0, 1.0),
        // El "drop" del demo cae al 40% y el regreso final al 80%.
        climax: [0.40, 0.80].iter().any(|c| p >= *c && p - dt / len < *c),
        section: [0.12, 0.40, 0.72, 0.80].iter().any(|c| p >= *c && p - dt / len < *c),
        beat: crossed(phase, 112.0 / 60.0),
        beat_phase: phase,
        pan: 0.5 * (t * 0.4).sin(),
        song: Song::default(),
    }
}

/// Perfil de canción inventado para el demo, con el ánimo pedido.
pub fn song_for(mood: Mood) -> Song {
    let (valence, arousal, bpm, key, minor) = match mood {
        Mood::Happy => (0.78, 0.76, 124.0, 7, false),
        Mood::Serene => (0.72, 0.28, 78.0, 5, false),
        Mood::Nostalgic => (0.48, 0.44, 96.0, 9, true),
        Mood::Sad => (0.22, 0.22, 68.0, 2, true),
        Mood::Intense => (0.22, 0.80, 150.0, 4, true),
    };
    Song {
        bpm,
        tempo_confidence: 0.8,
        key,
        minor,
        key_confidence: 0.8,
        majorness: if minor { -0.7 } else { 0.7 },
        brightness: 0.3 + 0.4 * valence,
        density: arousal,
        valence,
        arousal,
        settled: 1.0,
    }
}

pub struct DemoSession {
    t: f32,
    len: f32,
    paused: bool,
    info: TrackInfo,
    /// Cada vuelta del demo muestra un ánimo distinto.
    round: usize,
}

impl DemoSession {
    pub fn new(len: f32) -> Self {
        DemoSession {
            t: 0.0,
            len,
            paused: false,
            round: 0,
            info: TrackInfo {
                title: "Demo".into(),
                artist: "señal sintética, sin audio".into(),
                duration: Some(Duration::from_secs_f32(len)),
                known: false,
            },
        }
    }
}

impl Session for DemoSession {
    fn tick(&mut self, dt: f32) -> Tick {
        if self.paused {
            return Tick::default();
        }
        self.t += dt;
        let mut new_track = false;
        if self.t >= self.len {
            self.t = 0.0;
            self.round += 1;
            new_track = true;
        }
        let mut bands = bands_at(self.t, self.len, dt);
        // Recorre los cinco ánimos, uno por vuelta, empezando por la noche con luna.
        const TOUR: [Mood; 5] = [Mood::Nostalgic, Mood::Happy, Mood::Sad, Mood::Serene, Mood::Intense];
        bands.song = song_for(TOUR[self.round % TOUR.len()]);
        Tick { bands, new_track, ..Tick::default() }
    }
    fn track(&self) -> &TrackInfo {
        &self.info
    }
    fn position(&self) -> Duration {
        Duration::from_secs_f32(self.t)
    }
    fn playing(&self) -> bool {
        !self.paused
    }
    fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }
    fn change_volume(&mut self, _steps: i32) {}
    fn label(&self) -> String {
        "demo".into()
    }
}

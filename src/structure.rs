//! Mapa de la canción: pre-análisis del archivo completo para saber dónde están los clímax.
//!
//! En modo archivo se conoce toda la canción de antemano. Un hilo la decodifica entera (tarda
//! cerca de un segundo), mide el volumen cada 0,25 s y marca los pasajes más intensos. Así la
//! intensidad es relativa a la canción completa y los clímax caen en su instante exacto.
//! En modo Spotify no hay futuro que mirar: se usa el estimador en tiempo real
//! (`analysis::Dynamics`).

use std::fs::File;
use std::path::Path;

use rodio::{Decoder, Source};

use crate::features::{Listener, Song};

/// Segundos entre mediciones de volumen.
pub const HOP: f32 = 0.25;

pub struct SongMap {
    /// Intensidad 0..1 cada `HOP` segundos.
    intensity: Vec<f32>,
    /// Instantes (s) donde empieza un clímax.
    pub climaxes: Vec<f32>,
    /// Instantes (s) donde cambia la sección: el volumen sube o baja de golpe.
    pub sections: Vec<f32>,
    /// Tempo, tonalidad y ánimo de la canción completa.
    pub song: Song,
}

impl SongMap {
    /// Decodifica el archivo completo y arma el mapa. `None` si no se puede leer.
    pub fn analyze_file(path: &Path) -> Option<SongMap> {
        let decoder = Decoder::try_from(File::open(path).ok()?).ok()?;
        let channels = decoder.channels().get() as usize;
        let rate = decoder.sample_rate().get();
        let hop_samples = ((rate as f32 * HOP) as usize).max(1) * channels;
        let mut loudness = Vec::new();
        let (mut sum, mut n) = (0.0f64, 0usize);
        // El mismo oyente del modo en vivo, pero sobre la canción entera.
        let mut listener = Listener::new(rate);
        let mut mono = Vec::with_capacity(hop_samples / channels);
        let (mut frame, mut index) = (0.0f32, 0usize);
        let mut tempos = Vec::new();
        for s in decoder {
            sum += (s as f64) * (s as f64);
            n += 1;
            frame += s;
            index += 1;
            if index == channels {
                mono.push(frame / channels as f32);
                frame = 0.0;
                index = 0;
            }
            if n == hop_samples {
                loudness.push(to_db((sum / n as f64).sqrt() as f32));
                sum = 0.0;
                n = 0;
                listener.push(&mono);
                mono.clear();
                let now = listener.song();
                if now.tempo_confidence > 0.3 && now.bpm > 0.0 {
                    tempos.push(now.bpm);
                }
            }
        }
        if loudness.len() <= 8 {
            return None;
        }
        let mut map = SongMap::from_loudness(&loudness);
        map.song = listener.song();
        map.song.settled = 1.0;
        // Tempo de la canción: la mediana de lo medido a lo largo de ella.
        if !tempos.is_empty() {
            tempos.sort_by(f32::total_cmp);
            map.song.bpm = tempos[tempos.len() / 2];
        }
        Some(map)
    }

    /// Arma el mapa desde el volumen en dB medido cada `HOP` segundos.
    pub fn from_loudness(db: &[f32]) -> SongMap {
        // Rango de la canción: percentiles 15 y 92 de lo que no es silencio.
        let mut audible: Vec<f32> = db.iter().copied().filter(|v| *v > -55.0).collect();
        if audible.len() < 4 {
            return SongMap { intensity: vec![0.0; db.len()], climaxes: Vec::new(), sections: Vec::new(), song: Song::default() };
        }
        audible.sort_by(f32::total_cmp);
        let pick = |q: f32| audible[((audible.len() - 1) as f32 * q) as usize];
        let (lo, hi) = (pick(0.15), pick(0.92));
        let span = (hi - lo).max(4.0);

        let smooth = moving_average(db, (3.0 / HOP) as usize);
        let intensity: Vec<f32> = smooth.iter().map(|v| ((v - lo) / span).clamp(0.0, 1.0)).collect();

        // Clímax: la intensidad sube sobre 0,72 y se mantiene al menos 5 s, viniendo de un
        // pasaje más tranquilo. Sale del estado al bajar de 0,55.
        let fine = moving_average(db, (0.5 / HOP) as usize);
        let secs = |s: f32| (s / HOP) as usize;
        let mut climaxes = Vec::new();
        let mut i = 0;
        while i < intensity.len() {
            if intensity[i] < 0.72 {
                i += 1;
                continue;
            }
            let start = i;
            while i < intensity.len() && intensity[i] >= 0.55 {
                i += 1;
            }
            let long_enough = i - start >= secs(5.0);
            let before = &intensity[start.saturating_sub(secs(5.0))..start.saturating_sub(secs(1.0))];
            let came_from_calm =
                !before.is_empty() && before.iter().sum::<f32>() / (before.len() as f32) < 0.6;
            if long_enough && came_from_calm {
                // Afina el instante: el mayor salto de volumen en ±2 s alrededor del cruce.
                let (a, b) = (start.saturating_sub(secs(2.0)).max(1), (start + secs(2.0)).min(fine.len() - 1));
                let jump = (a..=b)
                    .max_by(|x, y| (fine[*x] - fine[*x - 1]).total_cmp(&(fine[*y] - fine[*y - 1])))
                    .unwrap_or(start);
                climaxes.push(jump as f32 * HOP);
            }
        }
        // Cambios de sección: la intensidad se mueve más de 0,3 en 3 s, hacia arriba o hacia
        // abajo. Mínimo 8 s entre uno y otro.
        let mut sections: Vec<f32> = Vec::new();
        let span = secs(3.0).max(1);
        for i in span..intensity.len() {
            let t = i as f32 * HOP;
            let moved = (intensity[i] - intensity[i - span]).abs() > 0.3;
            if moved && sections.last().is_none_or(|last| t - last > 8.0) {
                sections.push(t - 1.5);
            }
        }
        SongMap { intensity, climaxes, sections, song: Song::default() }
    }

    /// Intensidad 0..1 en el segundo `t`.
    pub fn intensity_at(&self, t: f32) -> f32 {
        if self.intensity.is_empty() {
            return 0.0;
        }
        let x = (t / HOP).max(0.0);
        let i = (x as usize).min(self.intensity.len() - 1);
        let j = (i + 1).min(self.intensity.len() - 1);
        self.intensity[i] + (self.intensity[j] - self.intensity[i]) * x.fract()
    }

    /// ¿Cambia la sección entre `t0` (exclusivo) y `t1` (inclusivo)?
    pub fn section_between(&self, t0: f32, t1: f32) -> bool {
        self.sections.iter().any(|c| *c > t0 && *c <= t1)
    }

    /// ¿Empieza un clímax entre `t0` (exclusivo) y `t1` (inclusivo)?
    pub fn climax_between(&self, t0: f32, t1: f32) -> bool {
        self.climaxes.iter().any(|c| *c > t0 && *c <= t1)
    }
}

fn to_db(rms: f32) -> f32 {
    20.0 * rms.max(1e-6).log10()
}

/// Promedio móvil centrado de `width` muestras.
fn moving_average(values: &[f32], width: usize) -> Vec<f32> {
    let half = width.max(1) / 2;
    (0..values.len())
        .map(|i| {
            let (a, b) = (i.saturating_sub(half), (i + half + 1).min(values.len()));
            values[a..b].iter().sum::<f32>() / (b - a) as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Canción sintética: intro suave, verso, coro fuerte, puente, coro final.
    fn song() -> Vec<f32> {
        let sections: [(f32, f32); 6] =
            [(15.0, -30.0), (35.0, -20.0), (25.0, -11.0), (20.0, -22.0), (30.0, -10.0), (10.0, -28.0)];
        let mut db = Vec::new();
        for (secs, level) in sections {
            for k in 0..(secs / HOP) as usize {
                db.push(level + 0.8 * (k as f32 * 0.7).sin());
            }
        }
        db
    }

    #[test]
    fn finds_both_choruses() {
        let map = SongMap::from_loudness(&song());
        // Los coros empiezan en los segundos 50 y 95.
        assert_eq!(map.climaxes.len(), 2, "{:?}", map.climaxes);
        assert!((map.climaxes[0] - 50.0).abs() <= 0.5, "{:?}", map.climaxes);
        assert!((map.climaxes[1] - 95.0).abs() <= 0.5, "{:?}", map.climaxes);
    }

    #[test]
    fn intensity_follows_sections() {
        let map = SongMap::from_loudness(&song());
        assert!(map.intensity_at(8.0) < 0.1, "intro {}", map.intensity_at(8.0));
        assert!(map.intensity_at(60.0) > 0.9, "coro {}", map.intensity_at(60.0));
        let bridge = map.intensity_at(85.0);
        assert!((0.1..0.6).contains(&bridge), "puente {bridge}");
    }

    #[test]
    fn climax_between_is_edge_exact() {
        let map = SongMap { intensity: vec![0.0; 10], climaxes: vec![50.0], sections: Vec::new(), song: Song::default() };
        assert!(map.climax_between(49.9, 50.0));
        assert!(!map.climax_between(50.0, 50.1));
    }

    /// Archivo real (solo con `DISKTUI_TEST_MEDIA`): song.mp3 dura 60 s y sube de volumen
    /// en los segundos 20 y 45.
    #[test]
    fn analyzes_a_real_file() {
        let Ok(dir) = std::env::var("DISKTUI_TEST_MEDIA") else { return };
        let started = std::time::Instant::now();
        let map = SongMap::analyze_file(&Path::new(&dir).join("song.mp3")).unwrap();
        assert!(started.elapsed().as_secs_f32() < 3.0, "tardó {:?}", started.elapsed());
        assert_eq!(map.climaxes.len(), 2, "{:?}", map.climaxes);
        assert!((map.climaxes[0] - 20.0).abs() < 0.6 && (map.climaxes[1] - 45.0).abs() < 0.6, "{:?}", map.climaxes);
        assert!(map.intensity_at(10.0) < 0.2 && map.intensity_at(28.0) > 0.8);
        // Sube en 20, baja en 35, sube en 45: tres cambios de sección.
        assert_eq!(map.sections.len(), 3, "{:?}", map.sections);
    }

    /// test.mp3: bombo a 120 BPM y un solo acorde (La, Do♯, Mi). Un acorde suelto no
    /// define una tonalidad (cabe en La mayor, Do♯ menor y Mi mayor), así que solo se exige
    /// que la tónica sea una de sus tres notas. La distinción mayor/menor con progresiones
    /// completas se prueba en `features`.
    #[test]
    fn reads_tempo_and_key_from_a_real_file() {
        let Ok(dir) = std::env::var("DISKTUI_TEST_MEDIA") else { return };
        let map = SongMap::analyze_file(&Path::new(&dir).join("test.mp3")).unwrap();
        let song = map.song;
        assert!((song.bpm - 120.0).abs() < 2.0, "tempo {}", song.bpm);
        assert!([9, 1, 4].contains(&song.key), "tonalidad {}", song.key_name());
        assert_eq!(song.settled, 1.0);
    }

    #[test]
    fn flat_song_has_no_climax() {
        let db: Vec<f32> = (0..800).map(|k| -12.0 + 0.5 * (k as f32 * 0.3).sin()).collect();
        assert!(SongMap::from_loudness(&db).climaxes.is_empty());
    }
}

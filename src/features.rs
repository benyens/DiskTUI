//! Entender la canción: pulso, tonalidad y ánimo.
//!
//! Tres análisis clásicos de señal, sin modelos entrenados, que corren en tiempo real:
//!
//! - **Pulso.** Envolvente de ataques por flujo espectral (100 veces por segundo),
//!   autocorrelación para el tempo y un peine para la fase. Predice el próximo golpe en vez
//!   de reaccionar a él.
//! - **Tonalidad.** Croma de 12 notas desde una FFT larga, comparado con los perfiles de
//!   Krumhansl-Kessler para las 24 tonalidades.
//! - **Ánimo.** Valencia (modo mayor o menor, brillo) y activación (tempo, densidad de
//!   ataques), al estilo del modelo circunflejo de Russell. Es una heurística: acierta el
//!   carácter general de una canción, no sus matices.

use std::collections::VecDeque;
use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};

/// Mediciones de ataques por segundo.
pub const HOP_HZ: f32 = 100.0;
const FLUX_FFT: usize = 1024;
const CHROMA_FFT: usize = 8192;
/// Historial de la envolvente de ataques: 8 segundos.
const HISTORY: usize = 800;
/// Rango de tempos buscados, en pulsos por minuto.
const BPM_RANGE: (f32, f32) = (55.0, 200.0);

/// Nombres de las 12 notas, de Do a Si.
pub const NOTES: [&str; 12] = ["Do", "Do♯", "Re", "Mi♭", "Mi", "Fa", "Fa♯", "Sol", "La♭", "La", "Si♭", "Si"];

/// Lo que se sabe de la canción hasta ahora.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Song {
    /// Tempo en pulsos por minuto. 0 = aún desconocido.
    pub bpm: f32,
    /// Qué tan marcado es el pulso, 0..1.
    pub tempo_confidence: f32,
    /// Tónica: 0 = Do ... 11 = Si.
    pub key: u8,
    pub minor: bool,
    pub key_confidence: f32,
    /// −1 claramente menor, +1 claramente mayor.
    pub majorness: f32,
    /// Brillo del timbre, 0..1.
    pub brightness: f32,
    /// Ataques por segundo, normalizado a 0..1.
    pub density: f32,
    /// 0 triste, 1 alegre.
    pub valence: f32,
    /// 0 calmada, 1 enérgica.
    pub arousal: f32,
    /// Cuánto fiarse de todo lo anterior, 0..1: sube en los primeros 20 s de escucha.
    pub settled: f32,
}

impl Song {
    /// Tonalidad en texto: "La menor", "Do mayor".
    pub fn key_name(&self) -> String {
        format!("{} {}", NOTES[self.key as usize % 12], if self.minor { "menor" } else { "mayor" })
    }
}

/// Perfiles de Krumhansl-Kessler: cuánto "pesa" cada nota en una tonalidad mayor y menor.
const MAJOR: [f32; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const MINOR: [f32; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];

fn correlation(a: &[f32; 12], b: &[f32; 12], shift: usize) -> f32 {
    let (ma, mb) = (a.iter().sum::<f32>() / 12.0, b.iter().sum::<f32>() / 12.0);
    let (mut num, mut da, mut db) = (0.0, 0.0, 0.0);
    for i in 0..12 {
        let (x, y) = (a[(i + shift) % 12] - ma, b[i] - mb);
        num += x * y;
        da += x * x;
        db += y * y;
    }
    num / (da * db).sqrt().max(1e-9)
}

/// Tonalidad más probable para un croma: (tónica, es menor, confianza 0..1, mayor-menor −1..1).
pub fn estimate_key(chroma: &[f32; 12]) -> (u8, bool, f32, f32) {
    if chroma.iter().sum::<f32>() < 1e-6 {
        return (0, false, 0.0, 0.0);
    }
    let (mut best_major, mut best_minor) = ((0usize, -1.0f32), (0usize, -1.0f32));
    let mut all = Vec::with_capacity(24);
    for tonic in 0..12 {
        let (ma, mi) = (correlation(chroma, &MAJOR, tonic), correlation(chroma, &MINOR, tonic));
        all.push(ma);
        all.push(mi);
        if ma > best_major.1 {
            best_major = (tonic, ma);
        }
        if mi > best_minor.1 {
            best_minor = (tonic, mi);
        }
    }
    let minor = best_minor.1 > best_major.1;
    let (tonic, best) = if minor { best_minor } else { best_major };
    let mean = all.iter().sum::<f32>() / 24.0;
    let confidence = ((best - mean) / 0.6).clamp(0.0, 1.0);
    let majorness = ((best_major.1 - best_minor.1) * 5.0).clamp(-1.0, 1.0);
    (tonic as u8, minor, confidence, majorness)
}

/// Ánimo a partir de las mediciones: (valencia, activación), ambas 0..1.
pub fn mood(bpm: f32, tempo_confidence: f32, majorness: f32, key_confidence: f32, brightness: f32, density: f32) -> (f32, f32) {
    let tempo = if tempo_confidence > 0.2 && bpm > 0.0 { ((bpm - 60.0) / 100.0).clamp(0.0, 1.0) } else { 0.4 };
    let arousal = (0.40 * tempo + 0.35 * density + 0.25 * brightness).clamp(0.0, 1.0);
    let valence = (0.5
        + 0.32 * majorness * (0.4 + 0.6 * key_confidence)
        + 0.20 * (brightness - 0.5)
        + 0.12 * (tempo - 0.5))
        .clamp(0.0, 1.0);
    (valence, arousal)
}

/// Escucha el audio y mantiene al día pulso, tonalidad y ánimo.
pub struct Listener {
    sample_rate: f32,
    hop: usize,
    /// Muestras que faltan para completar el próximo salto.
    until_hop: usize,
    ring: Vec<f32>,
    ring_pos: usize,
    window: Vec<f32>,
    fft: Arc<dyn RealToComplex<f32>>,
    input: Vec<f32>,
    spectrum: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    prev_mag: Vec<f32>,
    flux_mean: f32,
    /// Envolvente de ataques, una muestra por salto.
    onset: VecDeque<f32>,
    /// Lo mismo, solo en graves (bombo y bajo): decide dónde cae el pulso.
    onset_low: VecDeque<f32>,
    low_mean: f32,
    /// Saltos procesados: el reloj de audio.
    hops: u64,

    // Pulso.
    period: f32,
    confidence: f32,
    next_beat: f64,
    last_beat: f64,
    beat_flag: bool,
    candidate: (f32, u32),

    // Tonalidad.
    big_ring: Vec<f32>,
    big_pos: usize,
    big_window: Vec<f32>,
    big_fft: Arc<dyn RealToComplex<f32>>,
    big_input: Vec<f32>,
    big_spectrum: Vec<Complex<f32>>,
    big_scratch: Vec<Complex<f32>>,
    chroma: [f32; 12],

    // Estadísticas para el ánimo.
    brightness: f32,
    density: f32,
    since_onset: u32,
    active_hops: u32,
}

impl Listener {
    pub fn new(sample_rate: u32) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FLUX_FFT);
        let big_fft = planner.plan_fft_forward(CHROMA_FFT);
        let hann = |n: usize| -> Vec<f32> {
            (0..n).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (n - 1) as f32).cos()).collect()
        };
        let sr = sample_rate.max(8000) as f32;
        let hop = (sr / HOP_HZ).round() as usize;
        Listener {
            sample_rate: sr,
            hop,
            until_hop: hop,
            ring: vec![0.0; FLUX_FFT],
            ring_pos: 0,
            window: hann(FLUX_FFT),
            input: fft.make_input_vec(),
            spectrum: fft.make_output_vec(),
            scratch: fft.make_scratch_vec(),
            prev_mag: vec![0.0; FLUX_FFT / 2 + 1],
            fft,
            flux_mean: 0.0,
            onset: VecDeque::with_capacity(HISTORY),
            onset_low: VecDeque::with_capacity(HISTORY),
            low_mean: 0.0,
            hops: 0,
            period: 0.0,
            confidence: 0.0,
            next_beat: 0.0,
            last_beat: 0.0,
            beat_flag: false,
            candidate: (0.0, 0),
            big_ring: vec![0.0; CHROMA_FFT],
            big_pos: 0,
            big_window: hann(CHROMA_FFT),
            big_input: big_fft.make_input_vec(),
            big_spectrum: big_fft.make_output_vec(),
            big_scratch: big_fft.make_scratch_vec(),
            big_fft,
            chroma: [0.0; 12],
            brightness: 0.5,
            density: 0.0,
            since_onset: 0,
            active_hops: 0,
        }
    }

    /// Canción nueva: se olvida todo lo aprendido.
    pub fn reset(&mut self) {
        *self = Listener::new(self.sample_rate as u32);
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        if (sample_rate.max(8000) as f32 - self.sample_rate).abs() > 1.0 {
            *self = Listener::new(sample_rate);
        }
    }

    pub fn push(&mut self, samples: &[f32]) {
        for &s in samples {
            self.ring[self.ring_pos] = s;
            self.ring_pos = (self.ring_pos + 1) % FLUX_FFT;
            self.big_ring[self.big_pos] = s;
            self.big_pos = (self.big_pos + 1) % CHROMA_FFT;
            self.until_hop -= 1;
            if self.until_hop == 0 {
                self.until_hop = self.hop;
                self.step();
            }
        }
    }

    /// ¿Pasó un pulso desde la última consulta?
    pub fn take_beat(&mut self) -> bool {
        std::mem::take(&mut self.beat_flag)
    }

    /// Posición dentro del pulso actual: 0 justo en el golpe, cerca de 1 antes del siguiente.
    pub fn beat_phase(&self) -> f32 {
        if self.period <= 0.0 {
            return 0.0;
        }
        (((self.hops as f64 - self.last_beat) / self.period as f64) as f32).clamp(0.0, 1.0)
    }

    pub fn song(&self) -> Song {
        let bpm = if self.period > 0.0 { 60.0 * HOP_HZ / self.period } else { 0.0 };
        let (key, minor, key_confidence, majorness) = estimate_key(&self.chroma);
        let (valence, arousal) =
            mood(bpm, self.confidence, majorness, key_confidence, self.brightness, self.density);
        Song {
            bpm,
            tempo_confidence: self.confidence,
            key,
            minor,
            key_confidence,
            majorness,
            brightness: self.brightness,
            density: self.density,
            valence,
            arousal,
            settled: (self.active_hops as f32 / (20.0 * HOP_HZ)).min(1.0),
        }
    }

    /// Un salto de audio: mide el ataque, avanza el reloj y, cada tanto, tempo y croma.
    fn step(&mut self) {
        self.hops += 1;
        let mut energy = 0.0f32;
        for i in 0..FLUX_FFT {
            let s = self.ring[(self.ring_pos + i) % FLUX_FFT];
            energy += s * s;
            self.input[i] = s * self.window[i];
        }
        let rms = (energy / FLUX_FFT as f32).sqrt();
        let silent = rms < 1e-4;
        let mut flux = 0.0f32;
        let mut low_flux = 0.0f32;
        if self.fft.process_with_scratch(&mut self.input, &mut self.spectrum, &mut self.scratch).is_ok() {
            let hz = self.sample_rate / FLUX_FFT as f32;
            let (lo, hi) = ((30.0 / hz) as usize + 1, ((8000.0 / hz) as usize).min(self.prev_mag.len()));
            let (mut weighted, mut total) = (0.0f32, 0.0f32);
            for k in lo..hi {
                let raw = self.spectrum[k].norm() * 4.0 / FLUX_FFT as f32;
                // Compresión logarítmica: los ataques suaves también cuentan.
                let mag = (1.0 + 100.0 * raw).ln();
                let rise = (mag - self.prev_mag[k]).max(0.0);
                flux += rise;
                if (k as f32) * hz < 200.0 {
                    low_flux += rise;
                }
                self.prev_mag[k] = mag;
                weighted += raw * k as f32 * hz;
                total += raw;
            }
            if !silent && total > 1e-6 {
                // Centroide espectral en escala logarítmica: 500 Hz = opaco, 4 kHz = brillante.
                let centroid = weighted / total;
                let bright = ((centroid.max(1.0).log2() - 500f32.log2()) / 3.0).clamp(0.0, 1.0);
                self.brightness += (bright - self.brightness) * 0.002;
            }
        }
        if silent {
            flux = 0.0;
            low_flux = 0.0;
        } else {
            self.active_hops = self.active_hops.saturating_add(1);
        }

        // Envolvente normalizada por su media reciente, para no depender del volumen.
        self.flux_mean += (flux - self.flux_mean) * 0.01;
        let value = if self.flux_mean > 1e-4 { flux / self.flux_mean } else { 0.0 };
        if self.onset.len() == HISTORY {
            self.onset.pop_front();
        }
        self.onset.push_back(value);
        self.low_mean += (low_flux - self.low_mean) * 0.01;
        if self.onset_low.len() == HISTORY {
            self.onset_low.pop_front();
        }
        self.onset_low.push_back(if self.low_mean > 1e-4 { low_flux / self.low_mean } else { 0.0 });

        // Densidad de ataques: picos claros de la envolvente por segundo.
        self.since_onset += 1;
        let is_onset = value > 2.2 && self.since_onset > 6;
        if is_onset {
            self.since_onset = 0;
        }
        let rate = if is_onset { HOP_HZ } else { 0.0 };
        let per_second = self.density * 7.0 + (rate - self.density * 7.0) * 0.003;
        self.density = (per_second / 7.0).clamp(0.0, 1.0);

        // Pulso: dispara cuando el reloj alcanza el golpe previsto.
        if self.period > 0.0 && self.hops as f64 >= self.next_beat {
            self.last_beat = self.next_beat;
            self.next_beat += self.period as f64;
            if self.confidence > 0.2 {
                self.beat_flag = true;
            }
        }
        if self.hops.is_multiple_of(50) && self.onset.len() >= 300 {
            self.track_tempo();
        }
        if self.hops.is_multiple_of(20) && !silent {
            self.update_chroma();
        }
    }

    /// Tempo por autocorrelación de la envolvente y fase por un peine de pulsos.
    fn track_tempo(&mut self) {
        let n = self.onset.len().min(600);
        let start = self.onset.len() - n;
        let mean = self.onset.iter().skip(start).sum::<f32>() / n as f32;
        let x: Vec<f32> = self.onset.iter().skip(start).map(|v| (v - mean).max(-mean)).collect();
        let acf = |lag: usize| -> f32 { (0..n - lag).map(|i| x[i] * x[i + lag]).sum::<f32>() / (n - lag) as f32 };
        let zero = acf(0).max(1e-6);

        let (min_lag, max_lag) = ((60.0 * HOP_HZ / BPM_RANGE.1) as usize, (60.0 * HOP_HZ / BPM_RANGE.0) as usize);
        let table: Vec<f32> = (0..=(max_lag * 2).min(n / 2)).map(acf).collect();
        // Un tempo real también tiene energía al doble y al triple del período.
        let score = |lag: usize| -> f32 {
            let at = |l: usize| table.get(l).copied().unwrap_or(0.0);
            let bpm = 60.0 * HOP_HZ / lag as f32;
            // Preferencia suave por tempos cercanos a 115: resuelve la ambigüedad de octava.
            let prior = (-0.5 * ((bpm / 115.0).log2() / 0.6).powi(2)).exp();
            (at(lag) + 0.5 * at(lag * 2) + 0.33 * at(lag * 3)) * prior
        };
        let best = (min_lag..=max_lag.min(table.len().saturating_sub(1)))
            .max_by(|a, b| score(*a).total_cmp(&score(*b)))
            .unwrap_or(min_lag);
        // Afinación parabólica: el pico real cae entre dos retardos enteros.
        let (a, b, c) = (score(best - 1), score(best), score(best + 1));
        let offset = if (a - 2.0 * b + c).abs() > 1e-9 { 0.5 * (a - c) / (a - 2.0 * b + c) } else { 0.0 };
        let period = best as f32 + offset.clamp(-0.5, 0.5);
        self.confidence = (table[best] / zero * 2.0).clamp(0.0, 1.0);

        // Un tempo nuevo debe repetirse dos veces antes de reemplazar al actual.
        if self.period <= 0.0 {
            self.period = period;
        } else if (period - self.period).abs() / self.period < 0.06 {
            self.period += 0.35 * (period - self.period);
            self.candidate = (0.0, 0);
        } else if (period - self.candidate.0).abs() / period < 0.06 {
            self.candidate.1 += 1;
            if self.candidate.1 >= 2 {
                self.period = period;
                self.candidate = (0.0, 0);
            }
        } else {
            self.candidate = (period, 0);
        }

        // Fase: el desfase cuyo peine de pulsos junta más energía en los últimos 4 s.
        let p = self.period;
        // Los graves pesan el triple: el pulso cae en el bombo, no en el platillo del
        // contratiempo, aunque el platillo tenga más flujo.
        let span = n.min(400);
        let low_start = self.onset_low.len() - span;
        let tail: Vec<f32> = x[n - span..]
            .iter()
            .zip(self.onset_low.iter().skip(low_start))
            .map(|(all, low)| all + 3.0 * low)
            .collect();
        let sample = |pos: f32| -> f32 {
            if pos < 0.0 || pos >= (span - 1) as f32 {
                return 0.0;
            }
            let i = pos as usize;
            tail[i] + (tail[i + 1] - tail[i]) * pos.fract()
        };
        let mut best_phase = (0.0f32, f32::MIN);
        let steps = (p * 2.0) as usize;
        for s in 0..steps {
            let phase = s as f32 * 0.5;
            let mut sum = 0.0;
            let mut k = 0;
            loop {
                let pos = (span - 1) as f32 - phase - k as f32 * p;
                if pos < 0.0 {
                    break;
                }
                // Los pulsos recientes pesan más.
                sum += sample(pos) * (1.0 - 0.5 * k as f32 * p / span as f32);
                k += 1;
            }
            if sum > best_phase.1 {
                best_phase = (phase, sum);
            }
        }
        let estimate = self.hops as f64 - best_phase.0 as f64 + p as f64;
        if self.next_beat <= 0.0 {
            self.next_beat = estimate;
            self.last_beat = estimate - p as f64;
        } else {
            // Corrección suave hacia la fase medida, por el camino más corto.
            let mut error = (estimate - self.next_beat) % p as f64;
            if error > p as f64 / 2.0 {
                error -= p as f64;
            } else if error < -(p as f64) / 2.0 {
                error += p as f64;
            }
            self.next_beat += 0.3 * error;
        }
    }

    /// Suma al croma la energía de cada nota en la ventana larga.
    fn update_chroma(&mut self) {
        for i in 0..CHROMA_FFT {
            self.big_input[i] = self.big_ring[(self.big_pos + i) % CHROMA_FFT] * self.big_window[i];
        }
        if self
            .big_fft
            .process_with_scratch(&mut self.big_input, &mut self.big_spectrum, &mut self.big_scratch)
            .is_err()
        {
            return;
        }
        let hz = self.sample_rate / CHROMA_FFT as f32;
        let mut frame = [0.0f32; 12];
        let (lo, hi) = ((65.0 / hz) as usize + 1, ((2100.0 / hz) as usize).min(self.big_spectrum.len()));
        for k in lo..hi {
            let freq = k as f32 * hz;
            // Nota más cercana: La 440 es la clase 9.
            let pitch = (12.0 * (freq / 440.0).log2()).round() as i32 + 9;
            frame[pitch.rem_euclid(12) as usize] += self.big_spectrum[k].norm_sqr();
        }
        let total: f32 = frame.iter().sum();
        if total < 1e-9 {
            return;
        }
        // Memoria larga (unos 40 s): la tonalidad es de la canción, no del compás.
        for (acc, v) in self.chroma.iter_mut().zip(frame) {
            *acc = *acc * 0.995 + v / total;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: u32 = 44100;

    /// Pista sintética: bombo en cada pulso, platillo en los contratiempos y un bajo.
    fn groove(bpm: f32, secs: f32) -> Vec<f32> {
        let beat = 60.0 / bpm;
        let mut seed = 12345u32;
        (0..(secs * SR as f32) as usize)
            .map(|n| {
                let t = n as f32 / SR as f32;
                let since = t % beat;
                let off = (t + beat / 2.0) % beat;
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let noise = (seed >> 9) as f32 / (1 << 23) as f32 - 1.0;
                0.8 * (-since * 25.0).exp() * (TAU * 60.0 * since).sin()
                    + 0.15 * (-off * 60.0).exp() * noise
                    + 0.1 * (TAU * 110.0 * t).sin()
            })
            .collect()
    }

    #[test]
    fn finds_the_tempo() {
        for bpm in [84.0, 100.0, 120.0, 128.0, 150.0] {
            let mut l = Listener::new(SR);
            l.push(&groove(bpm, 10.0));
            let song = l.song();
            assert!((song.bpm - bpm).abs() < 2.0, "{bpm} BPM: midió {}", song.bpm);
            assert!(song.tempo_confidence > 0.3, "{bpm} BPM: confianza {}", song.tempo_confidence);
        }
    }

    #[test]
    fn predicts_beats_on_time() {
        let bpm = 120.0;
        let audio = groove(bpm, 16.0);
        let mut l = Listener::new(SR);
        let mut beats = Vec::new();
        // Se alimenta en bloques de 10 ms y se anota cuándo dispara cada pulso.
        for (i, block) in audio.chunks(441).enumerate() {
            l.push(block);
            if l.take_beat() {
                beats.push((i + 1) as f32 * 0.01);
            }
        }
        let late: Vec<f32> = beats.iter().copied().filter(|t| *t > 8.0).collect();
        assert!((14..=18).contains(&late.len()), "pulsos en 8 s: {}", late.len());
        for t in &late {
            // Los bombos reales caen en múltiplos de 0,5 s.
            let error = (t % 0.5).min(0.5 - t % 0.5);
            assert!(error < 0.06, "pulso en {t:.2} s, a {error:.3} s del bombo real");
        }
    }

    #[test]
    fn silence_has_no_beat() {
        let mut l = Listener::new(SR);
        let mut beats = 0;
        for _ in 0..1000 {
            l.push(&[0.0; 441]);
            beats += l.take_beat() as u32;
        }
        assert_eq!(beats, 0);
        assert_eq!(l.song().settled, 0.0);
    }

    /// Acorde con sus tres primeros armónicos por nota.
    fn chord(notes: &[f32], secs: f32) -> Vec<f32> {
        (0..(secs * SR as f32) as usize)
            .map(|n| {
                let t = n as f32 / SR as f32;
                notes
                    .iter()
                    .map(|f| (TAU * f * t).sin() + 0.5 * (TAU * 2.0 * f * t).sin() + 0.25 * (TAU * 3.0 * f * t).sin())
                    .sum::<f32>()
                    * 0.1
            })
            .collect()
    }

    #[test]
    fn tells_major_from_minor() {
        // Do mayor: Do-Mi-Sol, Fa-La-Do, Sol-Si-Re.
        let mut l = Listener::new(SR);
        for notes in [[261.63, 329.63, 392.0], [349.23, 440.0, 523.25], [392.0, 493.88, 587.33], [261.63, 329.63, 392.0]] {
            l.push(&chord(&notes, 2.0));
        }
        let s = l.song();
        assert_eq!((s.key, s.minor), (0, false), "esperaba Do mayor, salió {}", s.key_name());
        assert!(s.majorness > 0.2);

        // La menor: La-Do-Mi, Re-Fa-La, Mi-Sol♯-Si.
        let mut l = Listener::new(SR);
        for notes in [[220.0, 261.63, 329.63], [293.66, 349.23, 440.0], [329.63, 415.3, 493.88], [220.0, 261.63, 329.63]] {
            l.push(&chord(&notes, 2.0));
        }
        let s = l.song();
        assert_eq!((s.key, s.minor), (9, true), "esperaba La menor, salió {}", s.key_name());
        assert!(s.majorness < -0.2);
    }

    #[test]
    fn mood_quadrants() {
        // Rápida, mayor y brillante: alegre. Lenta, menor y opaca: triste.
        let (v, a) = mood(140.0, 0.8, 0.8, 0.8, 0.75, 0.7);
        assert!(v > 0.65 && a > 0.65, "alegre: {v} {a}");
        let (v, a) = mood(68.0, 0.6, -0.8, 0.8, 0.25, 0.15);
        assert!(v < 0.35 && a < 0.35, "triste: {v} {a}");
        // Rápida y menor: intensa. Lenta y mayor: serena.
        let (v, a) = mood(150.0, 0.8, -0.8, 0.8, 0.5, 0.8);
        assert!(v < 0.45 && a > 0.6, "intensa: {v} {a}");
        let (v, a) = mood(72.0, 0.6, 0.8, 0.8, 0.5, 0.2);
        assert!(v > 0.55 && a < 0.4, "serena: {v} {a}");
    }

    #[test]
    fn fast_busy_track_reads_as_energetic() {
        let mut fast = Listener::new(SR);
        fast.push(&groove(150.0, 20.0));
        let mut slow = Listener::new(SR);
        slow.push(&groove(70.0, 20.0));
        assert!(fast.song().arousal > slow.song().arousal + 0.1, "{} vs {}", fast.song().arousal, slow.song().arousal);
    }
}

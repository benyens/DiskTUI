//! Análisis espectral: FFT en un hilo aparte, tres bandas enviadas por canal `mpsc`.
//!
//! Flujo: el audio (tap de rodio o captura de cpal) empuja muestras mono a un ring buffer
//! sin bloqueo. Este hilo las consume a ritmo de tiempo real, calcula la FFT 60 veces por
//! segundo y manda un [`Bands`] al hilo de la TUI.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};

use crate::features::{Listener, Song};

pub const FFT_SIZE: usize = 2048;
/// Análisis por segundo.
pub const ANALYSIS_HZ: f32 = 60.0;

/// Límites de banda en Hz.
const BASS: (f32, f32) = (20.0, 250.0);
const MID: (f32, f32) = (250.0, 4000.0);
const TREBLE: (f32, f32) = (4000.0, 20000.0);

/// Energía por banda, normalizada a 0..1, más golpes detectados.
#[derive(Clone, Copy, Debug, Default)]
pub struct Bands {
    /// Graves, 20–250 Hz.
    pub bass: f32,
    /// Medios, 250–4000 Hz.
    pub mid: f32,
    /// Agudos, 4000–20000 Hz.
    pub treble: f32,
    /// Volumen general.
    pub level: f32,
    /// Golpe de bombo detectado en este cuadro.
    pub kick: bool,
    /// Golpe de platillo / hi-hat detectado en este cuadro.
    pub hat: bool,
    /// Intensidad a largo plazo, 0..1: qué tan fuerte es este pasaje respecto al resto de
    /// la canción. A diferencia de las bandas, no se normaliza cuadro a cuadro.
    pub intensity: f32,
    /// Empieza un clímax (entra el coro, cae el drop) en este cuadro.
    pub climax: bool,
    /// Cambia la sección de la canción en este cuadro (sube o baja de golpe).
    pub section: bool,
    /// Pulso previsto: cae justo en este cuadro.
    pub beat: bool,
    /// Posición dentro del pulso: 0 en el golpe, cerca de 1 antes del siguiente.
    pub beat_phase: f32,
    /// Balance estéreo: −1 todo a la izquierda, +1 todo a la derecha.
    pub pan: f32,
    /// Tempo, tonalidad y ánimo estimados hasta ahora.
    pub song: Song,
}

/// Lo que comparten la fuente de audio y el hilo de análisis, además de las muestras.
#[derive(Clone)]
pub struct Link {
    /// Frecuencia de muestreo: la fija la fuente cuando la conoce.
    pub sample_rate: Arc<AtomicU32>,
    /// Balance estéreo −1..1 (bits de un `f32`): lo mide la fuente, que ve los dos canales.
    pub pan: Arc<AtomicU32>,
    /// Canción nueva: el análisis debe olvidar tempo, tonalidad y ánimo.
    pub reset: Arc<AtomicBool>,
}

impl Link {
    pub fn new(sample_rate: u32) -> Self {
        Link {
            sample_rate: Arc::new(AtomicU32::new(sample_rate)),
            pan: Arc::new(AtomicU32::new(0f32.to_bits())),
            reset: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// Mide el balance estéreo sobre la marcha y lo publica en el [`Link`] cada ~50 ms.
#[derive(Default)]
pub struct PanMeter {
    left: f32,
    right: f32,
    frames: u32,
}

impl PanMeter {
    /// Agrega un cuadro estéreo.
    #[inline]
    pub fn frame(&mut self, left: f32, right: f32, link: &Link) {
        self.left += left * left;
        self.right += right * right;
        self.frames += 1;
        if self.frames >= 2048 {
            let (l, r) = (self.left.sqrt(), self.right.sqrt());
            let pan = if l + r > 1e-6 { (r - l) / (r + l) } else { 0.0 };
            link.pan.store(pan.to_bits(), Ordering::Relaxed);
            *self = PanMeter::default();
        }
    }
}

/// Seguidor de una banda: normalización adaptativa, envolvente y detección de golpes.
struct Follower {
    /// Pico reciente: divide la señal para que canciones bajas y fuertes se vean parecido.
    peak: f32,
    env: f32,
    prev: [f32; 2],
    flux_avg: f32,
    cooldown: f32,
    release: f32,
    refractory: f32,
}

impl Follower {
    fn new(release: f32, refractory: f32) -> Self {
        Follower {
            peak: PEAK_FLOOR,
            env: 0.0,
            prev: [0.0; 2],
            flux_avg: 0.05,
            cooldown: 0.0,
            release,
            refractory,
        }
    }

    /// Devuelve (envolvente 0..1, hubo golpe).
    fn step(&mut self, raw: f32, dt: f32) -> (f32, bool) {
        // Normalización adaptativa: el pico cae lento (constante de 8 s).
        self.peak = (self.peak * (-dt / 8.0).exp()).max(raw).max(PEAK_FLOOR);
        let norm = ((raw / self.peak - 0.06) / 0.94).clamp(0.0, 1.0);

        // Envolvente: ataque rápido, caída suave.
        let tau = if norm > self.env { 0.012 } else { self.release };
        self.env += (norm - self.env) * (1.0 - (-dt / tau).exp());

        // Golpe: subida brusca respecto a los dos cuadros anteriores.
        let flux = (norm - self.prev[0].min(self.prev[1])).max(0.0);
        self.prev = [norm, self.prev[0]];
        self.cooldown -= dt;
        let hit = flux > 0.14 && flux > self.flux_avg * 1.9 + 0.03 && self.cooldown <= 0.0;
        if hit {
            self.cooldown = self.refractory;
        }
        self.flux_avg += (flux - self.flux_avg) * (1.0 - (-dt / 0.8).exp());
        (self.env, hit)
    }
}

/// Por debajo de esto se considera silencio (unos −48 dB).
const PEAK_FLOOR: f32 = 0.004;

/// Dinámica a largo plazo: detecta clímax en tiempo real, sin conocer el futuro.
///
/// Sigue el volumen real (en dB, sin normalizar) con dos promedios: uno rápido (~1 s) y
/// uno lento (~18 s). La intensidad combina dónde está el rápido dentro del rango visto
/// hasta ahora y cuánto supera al lento. Un clímax es el momento en que el rápido salta
/// más de 3 dB sobre el lento estando cerca del máximo conocido.
pub struct Dynamics {
    fast: f32,
    slow: f32,
    lo: f32,
    hi: f32,
    started: bool,
    above: bool,
    below: bool,
    /// En el último paso el volumen cayó de golpe.
    pub dropped: bool,
    cooldown: f32,
    intensity: f32,
}

impl Default for Dynamics {
    fn default() -> Self {
        Dynamics { fast: -60.0, slow: -60.0, lo: -60.0, hi: -60.0, started: false, above: false, below: false, dropped: false, cooldown: 0.0, intensity: 0.0 }
    }
}

impl Dynamics {
    /// `rms`: volumen crudo de la ventana actual. Devuelve (intensidad 0..1, empieza clímax).
    pub fn step(&mut self, rms: f32, dt: f32) -> (f32, bool) {
        let ease = |tau: f32| 1.0 - (-dt / tau).exp();
        self.cooldown -= dt;
        let db = 20.0 * rms.max(1e-6).log10();
        if db < -55.0 {
            // Silencio o pausa: la referencia no se mueve y la intensidad cae sola.
            self.intensity += (0.0 - self.intensity) * ease(1.5);
            return (self.intensity, false);
        }
        if !self.started {
            // Arranque neutro: aún no se sabe si esto es fuerte o suave para la canción.
            self.started = true;
            self.fast = db;
            self.slow = db;
            self.lo = db - 3.0;
            self.hi = db + 6.0;
        }
        self.fast += (db - self.fast) * ease(1.2);
        self.slow += (db - self.slow) * ease(18.0);
        // Rango visto: el máximo cede 3 dB por minuto, el mínimo sube igual de lento.
        self.hi = self.fast.max(self.hi - 0.05 * dt);
        self.lo = self.fast.min(self.lo + 0.05 * dt);

        let span = (self.hi - self.lo).max(6.0);
        let absolute = ((self.fast - self.lo) / span).clamp(0.0, 1.0);
        let relative = ((self.fast - self.slow + 1.0) / 6.0).clamp(0.0, 1.0);
        let target = 0.65 * absolute + 0.35 * relative;
        self.intensity += (target - self.intensity) * ease(0.6);

        // Caída brusca: el rápido queda 4 dB bajo el lento. Marca cambio de sección.
        let was_below = self.below;
        if self.fast - self.slow < -4.0 {
            self.below = true;
        } else if self.fast - self.slow > -2.0 {
            self.below = false;
        }
        self.dropped = self.below && !was_below;

        // Histéresis: entra sobre +3 dB, sale bajo +1,5 dB. Un clímax cada 10 s como máximo.
        let jump = self.fast - self.slow;
        let was_above = self.above;
        if jump > 3.0 && absolute > 0.7 {
            self.above = true;
        } else if jump < 1.5 {
            self.above = false;
        }
        let climax = self.above && !was_above && self.cooldown <= 0.0;
        if climax {
            self.cooldown = 10.0;
        }
        (self.intensity, climax)
    }
}

/// Convierte muestras mono en [`Bands`]. No sabe de hilos: se puede probar aislado.
pub struct Analyzer {
    sample_rate: f32,
    fft: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    /// Últimas `FFT_SIZE` muestras, circular.
    ring: Vec<f32>,
    ring_pos: usize,
    input: Vec<f32>,
    spectrum: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    bass: Follower,
    mid: Follower,
    treble: Follower,
    level: Follower,
    dynamics: Dynamics,
    listener: Listener,
    pan: f32,
}

impl Analyzer {
    pub fn new(sample_rate: u32) -> Self {
        let fft = RealFftPlanner::<f32>::new().plan_fft_forward(FFT_SIZE);
        // Ventana de Hann: reduce la fuga espectral entre bandas.
        let window = (0..FFT_SIZE)
            .map(|i| {
                let x = i as f32 / (FFT_SIZE - 1) as f32;
                0.5 - 0.5 * (std::f32::consts::TAU * x).cos()
            })
            .collect();
        Analyzer {
            sample_rate: sample_rate.max(8000) as f32,
            input: fft.make_input_vec(),
            spectrum: fft.make_output_vec(),
            scratch: fft.make_scratch_vec(),
            fft,
            window,
            ring: vec![0.0; FFT_SIZE],
            ring_pos: 0,
            bass: Follower::new(0.16, 0.16),
            mid: Follower::new(0.12, 0.12),
            treble: Follower::new(0.08, 0.07),
            level: Follower::new(0.20, 0.2),
            dynamics: Dynamics::default(),
            listener: Listener::new(sample_rate),
            pan: 0.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate.max(8000) as f32;
        self.listener.set_sample_rate(sample_rate);
    }

    /// Canción nueva: se olvida tempo, tonalidad, ánimo y la referencia de volumen.
    pub fn new_song(&mut self) {
        self.listener.reset();
        self.dynamics = Dynamics::default();
    }

    /// Balance estéreo medido por la fuente; aquí solo se suaviza.
    pub fn set_pan(&mut self, pan: f32) {
        self.pan += (pan.clamp(-1.0, 1.0) - self.pan) * 0.15;
    }

    pub fn push(&mut self, samples: &[f32]) {
        for &s in samples {
            self.ring[self.ring_pos] = s;
            self.ring_pos = (self.ring_pos + 1) % FFT_SIZE;
        }
        self.listener.push(samples);
    }

    /// Calcula las bandas sobre la ventana actual. `dt` = segundos desde el análisis anterior.
    pub fn analyze(&mut self, dt: f32) -> Bands {
        // Copia la ventana en orden temporal y aplica Hann.
        let mut sum_sq = 0.0f32;
        for i in 0..FFT_SIZE {
            let s = self.ring[(self.ring_pos + i) % FFT_SIZE];
            self.input[i] = s * self.window[i];
            if i >= FFT_SIZE / 2 {
                sum_sq += s * s;
            }
        }
        let rms = (sum_sq / (FFT_SIZE / 2) as f32).sqrt();

        if self
            .fft
            .process_with_scratch(&mut self.input, &mut self.spectrum, &mut self.scratch)
            .is_err()
        {
            return Bands::default();
        }

        // Una senoidal de amplitud A da un pico de A·N/4 con ventana de Hann.
        let norm = 4.0 / FFT_SIZE as f32;
        let hz_per_bin = self.sample_rate / FFT_SIZE as f32;
        let nyquist = self.sample_rate / 2.0;
        let band = |spectrum: &[Complex<f32>], (lo, hi): (f32, f32)| -> f32 {
            let a = ((lo / hz_per_bin).ceil() as usize).max(1);
            let b = ((hi.min(nyquist) / hz_per_bin).ceil() as usize).min(spectrum.len());
            if a >= b {
                return 0.0;
            }
            let energy: f32 = spectrum[a..b].iter().map(|c| c.norm_sqr()).sum();
            energy.sqrt() * norm
        };
        let raw_bass = band(&self.spectrum, BASS);
        let raw_mid = band(&self.spectrum, MID);
        let raw_treble = band(&self.spectrum, TREBLE);

        let (bass, kick) = self.bass.step(raw_bass, dt);
        let (mid, _) = self.mid.step(raw_mid, dt);
        let (treble, hat) = self.treble.step(raw_treble, dt);
        let (level, _) = self.level.step(rms, dt);
        let (intensity, climax) = self.dynamics.step(rms, dt);
        Bands {
            bass,
            mid,
            treble,
            level,
            kick,
            hat,
            intensity,
            climax,
            section: climax || self.dynamics.dropped,
            beat: self.listener.take_beat(),
            beat_phase: self.listener.beat_phase(),
            pan: self.pan,
            song: self.listener.song(),
        }
    }
}

/// Lanza el hilo de análisis.
///
/// - `rx`: muestras mono que llegan desde el audio.
/// - `link`: frecuencia de muestreo, balance estéreo y aviso de canción nueva.
/// - `tx`: canal hacia la TUI.
/// - `stop`: pon `true` para terminar el hilo.
pub fn spawn(
    mut rx: rtrb::Consumer<f32>,
    link: Link,
    tx: Sender<Bands>,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("disktui-fft".into())
        .spawn(move || {
            let mut sr = link.sample_rate.load(Ordering::Relaxed).max(8000);
            let mut analyzer = Analyzer::new(sr);
            // Cola intermedia: el audio llega a ráfagas (un bloque por callback) y aquí se
            // consume a ritmo constante, para que la animación no dé saltos.
            let mut fifo: VecDeque<f32> = VecDeque::with_capacity(sr as usize);
            let mut chunk: Vec<f32> = Vec::with_capacity(4096);
            let tick = Duration::from_secs_f32(1.0 / ANALYSIS_HZ);
            let mut next = Instant::now() + tick;
            let mut last = Instant::now();
            let mut owed = 0.0f32; // muestras pendientes de consumir (con fracción)
            let mut starved = 0.0f32; // segundos sin audio nuevo

            while !stop.load(Ordering::Relaxed) {
                let now = Instant::now();
                let dt = now.duration_since(last).as_secs_f32().clamp(0.001, 0.1);
                last = now;

                let cur = link.sample_rate.load(Ordering::Relaxed).max(8000);
                if cur != sr {
                    sr = cur;
                    analyzer.set_sample_rate(sr);
                }
                if link.reset.swap(false, Ordering::Relaxed) {
                    analyzer.new_song();
                }
                analyzer.set_pan(f32::from_bits(link.pan.load(Ordering::Relaxed)));

                while let Ok(s) = rx.pop() {
                    fifo.push_back(s);
                }
                // Acota el retraso: si se acumuló demasiado, salta hacia adelante.
                let max_backlog = (sr as f32 * 0.30) as usize;
                if fifo.len() > max_backlog {
                    let keep = (sr as f32 * 0.08) as usize;
                    fifo.drain(..fifo.len() - keep);
                }

                owed = (owed + sr as f32 * dt).min(FFT_SIZE as f32 * 2.0);
                let take = (owed as usize).min(fifo.len());
                chunk.clear();
                chunk.extend(fifo.drain(..take));
                if take > 0 {
                    owed -= take as f32;
                    starved = 0.0;
                    analyzer.push(&chunk);
                } else {
                    // Pausa o silencio: tras un cuarto de segundo, alimenta ceros para que
                    // las bandas caigan solas.
                    starved += dt;
                    if starved > 0.25 {
                        chunk.resize((sr as f32 * dt) as usize, 0.0);
                        analyzer.push(&chunk);
                    }
                    owed = owed.min(sr as f32 * dt);
                }

                if tx.send(analyzer.analyze(dt)).is_err() {
                    break; // la TUI cerró
                }

                let now = Instant::now();
                if next > now {
                    thread::sleep(next - now);
                    next += tick;
                } else {
                    next = now + tick;
                }
            }
        })
        .expect("no se pudo crear el hilo de análisis")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 44100;

    fn sine(freq: f32, amp: f32, n: usize, offset: usize) -> Vec<f32> {
        (0..n)
            .map(|i| amp * (std::f32::consts::TAU * freq * (i + offset) as f32 / SR as f32).sin())
            .collect()
    }

    /// Alimenta `secs` segundos de un tono y devuelve las bandas finales.
    fn run_tone(freq: f32, secs: f32) -> Bands {
        let mut a = Analyzer::new(SR);
        let hop = (SR as f32 / ANALYSIS_HZ) as usize;
        let mut out = Bands::default();
        for i in 0..(secs * ANALYSIS_HZ) as usize {
            a.push(&sine(freq, 0.5, hop, i * hop));
            out = a.analyze(1.0 / ANALYSIS_HZ);
        }
        out
    }

    #[test]
    fn tone_lands_in_its_band() {
        let b = run_tone(80.0, 1.0);
        assert!(b.bass > 0.8 && b.mid < 0.2 && b.treble < 0.2, "{b:?}");
        let b = run_tone(1000.0, 1.0);
        assert!(b.mid > 0.8 && b.bass < 0.2 && b.treble < 0.2, "{b:?}");
        let b = run_tone(8000.0, 1.0);
        assert!(b.treble > 0.8 && b.bass < 0.2 && b.mid < 0.2, "{b:?}");
    }

    #[test]
    fn silence_stays_at_zero() {
        let mut a = Analyzer::new(SR);
        let mut b = Bands::default();
        for _ in 0..120 {
            a.push(&vec![0.0; 735]);
            b = a.analyze(1.0 / ANALYSIS_HZ);
        }
        assert!(b.bass < 0.01 && b.mid < 0.01 && b.treble < 0.01 && b.level < 0.01, "{b:?}");
    }

    #[test]
    fn quiet_and_loud_look_alike() {
        // La normalización adaptativa iguala una misma señal a distinto volumen.
        let run = |amp: f32| {
            let mut a = Analyzer::new(SR);
            let hop = 735;
            let mut out = Bands::default();
            for i in 0..90 {
                a.push(&sine(100.0, amp, hop, i * hop));
                out = a.analyze(1.0 / ANALYSIS_HZ);
            }
            out.bass
        };
        assert!((run(0.05) - run(0.8)).abs() < 0.1);
    }

    #[test]
    fn climax_fires_when_the_song_gets_loud() {
        // 20 s suaves, 10 s fuertes, 10 s suaves. Un solo clímax, al entrar lo fuerte.
        let mut d = Dynamics::default();
        let dt = 1.0 / ANALYSIS_HZ;
        let mut climaxes = Vec::new();
        let (mut quiet_i, mut loud_i, mut after_i) = (0.0, 0.0, 0.0);
        for f in 0..(40.0 * ANALYSIS_HZ) as usize {
            let t = f as f32 * dt;
            let rms = if (20.0..30.0).contains(&t) { 0.30 } else { 0.05 };
            let (intensity, climax) = d.step(rms, dt);
            if climax {
                climaxes.push(t);
            }
            if (t - 19.0).abs() < dt { quiet_i = intensity; }
            if (t - 24.0).abs() < dt { loud_i = intensity; }
            if (t - 39.0).abs() < dt { after_i = intensity; }
        }
        assert_eq!(climaxes.len(), 1, "{climaxes:?}");
        assert!((20.0..22.0).contains(&climaxes[0]), "clímax en {climaxes:?}");
        assert!(loud_i > 0.8 && quiet_i < 0.5 && after_i < 0.3, "{quiet_i} {loud_i} {after_i}");
    }

    #[test]
    fn steady_volume_has_no_climax() {
        let mut d = Dynamics::default();
        let dt = 1.0 / ANALYSIS_HZ;
        let mut count = 0;
        for f in 0..(60.0 * ANALYSIS_HZ) as usize {
            // Volumen parejo con un vaivén de ±1 dB.
            let rms = 0.2 * (1.0 + 0.12 * (f as f32 * dt * 3.0).sin());
            count += d.step(rms, dt).1 as u32;
        }
        assert_eq!(count, 0);
    }

    #[test]
    fn detects_kicks() {
        // Bombo sintético a 120 BPM: ráfaga de 60 Hz con caída exponencial cada 0,5 s.
        let mut a = Analyzer::new(SR);
        let hop = 735usize;
        let mut kicks = 0;
        let total_frames = 4 * 60; // 4 segundos
        for f in 0..total_frames {
            let block: Vec<f32> = (0..hop)
                .map(|i| {
                    let n = f * hop + i;
                    let t = n as f32 / SR as f32;
                    let since = t % 0.5;
                    0.02 * (std::f32::consts::TAU * 300.0 * t).sin()
                        + 0.9 * (-since * 18.0).exp() * (std::f32::consts::TAU * 60.0 * since).sin()
                })
                .collect();
            a.push(&block);
            if a.analyze(1.0 / ANALYSIS_HZ).kick {
                kicks += 1;
            }
        }
        assert!((7..=9).contains(&kicks), "esperaba ~8 bombos, hubo {kicks}");
    }
}

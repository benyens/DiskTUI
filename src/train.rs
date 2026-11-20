//! Escena "tren nocturno": el paisaje pasa por la ventana de un tren.
//!
//! Los postes del tendido cruzan la ventana uno por pulso, el tren va más rápido con el
//! tempo y la energía, y al final de la canción frena en una estación. En los clímax pasa
//! un pueblo iluminado. El cielo y el clima son los del jardín ([`Scenery`]).

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::render::View;
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// Fracción del alto de la ventana donde está la línea de los cerros cercanos.
const HORIZON: f32 = 0.70;
/// La estación aparece en este punto de la canción y el tren se detiene al final.
const STATION_FROM: f32 = 0.93;
/// Duración de un pueblo iluminado tras un clímax, en segundos.
const TOWN_TIME: f32 = 5.0;

pub struct Train {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    progress: f32,
    bands: Bands,
    /// Distancia recorrida, en anchos de ventana.
    travel: f32,
    speed: f32,
    beat_pulse: f32,
    /// Fase del pulso propia, para cuando no hay tempo: avanza a 100 BPM.
    fallback_phase: f32,
    /// Pueblo iluminado: tiempo que le queda.
    town: f32,
    town_seed: u32,
    /// Canción nueva: la ventana se empaña y el viaje vuelve a empezar.
    fog: Option<f32>,
    seed: u32,
}

fn hash(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h >> 8) as f32 / (1 << 24) as f32
}

/// Ruido suave en 1D: interpolación entre valores al azar en los enteros.
fn noise(x: f32, seed: u32) -> f32 {
    let (i, f) = (x.floor(), x - x.floor());
    let (a, b) = (hash(i as i32 as u32, seed), hash((i as i32 + 1) as u32, seed));
    a + (b - a) * f * f * (3.0 - 2.0 * f)
}

impl Train {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0x7A41_0001);
        scenery.bare = true;
        let mut rng = Rng::new(seed ^ 0x0007_2A1B);
        let seed = (rng.next() * 1e6) as u32;
        Train {
            scenery,
            rng,
            time: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            travel: 0.0,
            speed: 0.0,
            beat_pulse: 0.0,
            fallback_phase: 0.0,
            town: 0.0,
            town_seed: 0,
            fog: None,
            seed,
        }
    }

    fn presence(&self) -> f32 {
        self.fog.map(|t| 1.0 - smoothstep(0.0, 0.6, t.min(1.0 - t) * 2.0)).unwrap_or(1.0)
    }

    /// Fase del pulso (0..1): la del análisis si hay tempo, si no una propia.
    fn phase(&self) -> f32 {
        if self.bands.song.bpm > 0.0 && self.bands.song.tempo_confidence > 0.2 { self.bands.beat_phase } else { self.fallback_phase }
    }

    /// Velocidad objetivo, en anchos de ventana por segundo.
    fn cruise(&self) -> f32 {
        let b = &self.bands;
        let tempo = if b.song.bpm > 0.0 { (b.song.bpm / 120.0).clamp(0.6, 1.5) } else { 1.0 };
        let depart = smoothstep(0.0, 0.05, self.progress);
        let arrive = 1.0 - smoothstep(STATION_FROM, 0.995, self.progress);
        (0.10 + 0.07 * b.intensity) * tempo * depart * arrive
    }

    fn advance(&mut self, f: &Frame) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        self.beat_pulse = (self.beat_pulse * (-dt / 0.25).exp()).max(if b.beat { 1.0 } else { 0.0 });
        self.fallback_phase = (self.fallback_phase + dt * 100.0 / 60.0).fract();

        if let Some(t) = &mut self.fog {
            *t += dt / 1.6;
            if *t >= 0.5 && self.progress > 0.0 {
                // A mitad del empañado, el viaje vuelve a empezar detrás del vidrio.
                self.progress = 0.0;
                self.travel = 0.0;
                self.speed = 0.0;
                self.seed = (self.rng.next() * 1e6) as u32;
            }
            if *t >= 1.0 {
                self.fog = None;
            }
        } else if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }

        let target = if f.playing { self.cruise() } else { 0.0 };
        self.speed += (target - self.speed) * (1.0 - (-dt / 1.5).exp());
        self.travel += self.speed * dt;
        if b.climax {
            self.town = TOWN_TIME;
            self.town_seed = (self.rng.next() * 1e6) as u32;
        }
        self.town = (self.town - dt).max(0.0);

        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    /// Borde de la ventana: grosor de marco y radio de las esquinas, en píxeles.
    fn frame_size(w: f32, h: f32) -> (f32, f32) {
        let border = (w.min(h * 2.0) * 0.035).clamp(2.0, 7.0);
        (border, border * 2.2)
    }

    /// Distancia al borde interior de la ventana (negativa dentro del vidrio).
    fn frame_distance(x: f32, y: f32, w: f32, h: f32) -> f32 {
        let (border, radius) = Self::frame_size(w, h);
        // Rectángulo redondeado: distancia con signo.
        let (hw, hh) = (w / 2.0 - border, h / 2.0 - border);
        let (dx, dy) = ((x - w / 2.0).abs() - (hw - radius), ((y - h / 2.0) * 2.0).abs() - (hh * 2.0 - radius));
        let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
        outside + dx.max(dy).min(0.0) - radius
    }

    fn draw_land(&self, canvas: &mut Canvas, horizon: f32, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let atm = self.scenery.atmosphere();
        let day = atm.daylight;
        let haze = self.scenery.sky(1.0);
        // Capas de lejos a cerca: cada una pasa más rápido (paralaje).
        let layers: [(f32, f32, f32, Rgb); 3] = [
            (0.06, 0.30, 0.22, Rgb(0.10, 0.12, 0.22).mix(Rgb(0.45, 0.52, 0.66), day)),
            (0.25, 0.16, 0.10, Rgb(0.05, 0.08, 0.10).mix(Rgb(0.22, 0.38, 0.30), day)),
            (1.00, 0.07, 0.04, Rgb(0.02, 0.03, 0.04).mix(Rgb(0.12, 0.24, 0.14), day)),
        ];
        for (n, (parallax, height, base, color)) in layers.iter().enumerate() {
            let color = color.mix(haze, 0.35 * (1.0 - n as f32 / 2.0)).mix(pal.leaf, 0.06);
            let shift = self.travel * parallax * w;
            for x in 0..canvas.w {
                let u = (x as f32 + shift) / w;
                let ridge = noise(u * (2.0 + 3.0 * n as f32), self.seed ^ n as u32) * 0.65 + noise(u * 9.0, self.seed ^ 77 ^ n as u32) * 0.35;
                let top = horizon - (base + height * ridge) * horizon;
                for y in top.max(0.0) as usize..h as usize {
                    canvas.paint(x as i32, y as i32, color, 1.0);
                }
                // Casas con luz en la capa del medio: más en los pueblos de los clímax.
                if n == 1 {
                    let cell = (u * 24.0).floor() as i32;
                    let town = smoothstep(0.0, 1.2, self.town.min(TOWN_TIME - self.town + 1.0)).min(1.0);
                    let chance = 0.10 + 0.55 * town;
                    let lot = hash(cell as u32, self.seed ^ (self.town_seed * u32::from(town > 0.0)));
                    if lot < chance && (u * 24.0).fract() < 0.30 {
                        let lit = (1.0 - day) * (0.6 + 0.4 * self.bands.mid);
                        let window = Rgb(1.0, 0.80, 0.45).mix(pal.accent, 0.3);
                        let y = (top + 1.0) as i32;
                        canvas.paint(x as i32, y, color.scale(1.4).mix(window, lit), 1.0);
                        canvas.glow(x as i32, y - 1, window, 0.12 * lit);
                    }
                }
            }
        }
    }

    /// Pasto junto a la vía: trazos que pasan rápido, para que se sienta la velocidad.
    fn draw_blur(&self, canvas: &mut Canvas, horizon: f32) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        if self.speed < 0.005 {
            return;
        }
        let atm = self.scenery.atmosphere();
        let blade = Rgb(0.06, 0.10, 0.07).mix(Rgb(0.30, 0.50, 0.26), atm.daylight);
        let near = horizon + (h - horizon) * 0.45;
        let shift = self.travel * 1.8 * w;
        let length = (self.speed * 60.0).clamp(1.0, 14.0);
        for row in near as i32..h as i32 {
            for k in 0..(w / 6.0) as i32 {
                let lot = hash(k as u32 ^ (row as u32).wrapping_mul(131), self.seed ^ 0xB1);
                if lot < 0.35 {
                    let x0 = (k as f32 * 6.0 + lot * 40.0 - shift).rem_euclid(w + 20.0) - 10.0;
                    for dx in 0..length as i32 {
                        let fade = 1.0 - dx as f32 / length;
                        canvas.blend(x0 as i32 + dx, row, blade.scale(1.1 + 0.4 * lot), 0.5 * fade);
                    }
                }
            }
        }
    }

    /// Postes del tendido: uno cruza el centro de la ventana en cada pulso.
    fn draw_poles(&self, canvas: &mut Canvas, horizon: f32, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        if self.speed < 0.005 {
            return;
        }
        let atm = self.scenery.atmosphere();
        let spacing = w * 0.55;
        let phase = self.phase();
        let wood = Rgb(0.05, 0.04, 0.04).mix(Rgb(0.30, 0.24, 0.18), atm.daylight);
        let top = horizon * 0.30;
        let mut tops = Vec::new();
        for k in -1..=2 {
            let x = w * 0.5 + (k as f32 - phase) * spacing;
            tops.push(x);
            for y in top as i32..h as i32 {
                canvas.paint(x as i32, y, wood, 1.0);
                canvas.paint(x as i32 + 1, y, wood.scale(0.8), 1.0);
            }
            // Travesaño con aisladores que brillan con cada pulso.
            for dx in -3..=4 {
                canvas.paint(x as i32 + dx, top as i32 + 1, wood, 1.0);
            }
            let spark = 0.15 + 0.6 * self.beat_pulse;
            canvas.glow(x as i32 - 3, top as i32, pal.accent, spark * (1.0 - atm.daylight * 0.7));
            canvas.glow(x as i32 + 4, top as i32, pal.accent, spark * (1.0 - atm.daylight * 0.7));
        }
        // Cables entre postes, con su comba.
        let wire = wood.mix(self.scenery.sky(0.5), 0.3);
        for pair in tops.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let steps = ((b - a) * 2.0) as i32;
            for (offset, sag) in [(-3.0, 0.10), (4.0, 0.12)] {
                for i in 0..=steps {
                    let t = i as f32 / steps.max(1) as f32;
                    let x = a + offset + (b - a) * t;
                    let y = top + 1.0 + sag * horizon * 4.0 * t * (1.0 - t);
                    // Los cables solo se ven por el vidrio, no sobre el marco.
                    if Self::frame_distance(x, y, w, h) < -0.5 {
                        canvas.dot_over((x * 2.0) as i32, (y * 2.0) as i32, wire, 0.9);
                    }
                }
            }
        }
    }

    fn draw_station(&self, canvas: &mut Canvas, horizon: f32, pal: &Palette) {
        let near = smoothstep(STATION_FROM, 1.0, self.progress);
        if near < 0.01 {
            return;
        }
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let atm = self.scenery.atmosphere();
        // El andén entra desde la derecha y queda frente a la ventana al detenerse.
        let x0 = w * (1.05 - 0.95 * near);
        let floor = horizon + (h - horizon) * 0.35;
        let concrete = Rgb(0.18, 0.18, 0.20).mix(Rgb(0.55, 0.55, 0.58), atm.daylight);
        for y in floor as i32..h as i32 {
            for x in x0 as i32..w as i32 {
                canvas.paint(x, y, concrete.scale(1.0 - 0.15 * ((y as f32 - floor) / (h - floor))), 1.0);
            }
        }
        // Faroles del andén y un letrero.
        let lamp = Rgb(1.0, 0.88, 0.60);
        for i in 0..4 {
            let x = x0 + w * (0.08 + 0.22 * i as f32);
            if x > w {
                break;
            }
            for y in (horizon * 0.45) as i32..floor as i32 {
                canvas.paint(x as i32, y, concrete.scale(0.5), 1.0);
            }
            let (lx, ly) = (x as i32, (horizon * 0.45) as i32);
            for dy in -4i32..=4 {
                for dx in -6i32..=6 {
                    let r2 = (dx * dx) as f32 / 4.0 + (dy * dy) as f32;
                    canvas.glow(lx + dx, ly + dy, lamp, 0.25 * (1.0 - atm.daylight * 0.8) * (-r2 / 6.0).exp());
                }
            }
        }
        let sign = pal.petal.scale(0.6 + 0.3 * atm.daylight);
        let (sx, sy) = (x0 + w * 0.15, horizon * 0.62);
        for y in sy as i32..(sy + 3.0) as i32 {
            for x in sx as i32..(sx + w * 0.14) as i32 {
                canvas.paint(x, y, sign, 1.0);
            }
        }
    }

    fn draw_window(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let atm = self.scenery.atmosphere();
        let wall = Rgb(0.10, 0.08, 0.08).mix(pal.soil, 0.35).mix(Rgb(0.32, 0.27, 0.24), atm.daylight * 0.6);
        let fog = 1.0 - self.presence();
        let glass = self.scenery.sky(0.3).mix(Rgb(0.75, 0.78, 0.82), 0.5);
        for y in 0..h as usize {
            for x in 0..canvas.w {
                let d = Self::frame_distance(x as f32 + 0.5, y as f32 + 0.5, w, h);
                if d > 0.0 {
                    // Marco: un poco de luz del vidrio en el borde interior.
                    let rim = (1.0 - d / 3.0).max(0.0) * 0.25;
                    canvas.solid(x as i32, y as i32, wall.add(glass, rim));
                } else {
                    // Vidrio: reflejo suave en diagonal, y vaho al cambiar de canción.
                    let streak = ((x as f32 * 0.6 - y as f32) / (w * 0.5)).fract();
                    let reflect = 0.035 * smoothstep(0.0, 0.08, streak) * (1.0 - smoothstep(0.08, 0.22, streak));
                    canvas.blend(x as i32, y as i32, glass, reflect + 0.85 * fog);
                }
            }
        }
    }
}

impl Scene for Train {
    fn update(&mut self, frame: &Frame, _w: usize, _h: usize) {
        self.advance(frame);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let ph = canvas.px_h();
        let horizon = (ph as f32 * HORIZON).floor().max(1.0);
        let view = View { cx: canvas.w as f32 / 2.0, ground: horizon, scale: horizon.max(4.0), weight: 1.0 };
        let scenery = &self.scenery;
        canvas.clear(|_, y| scenery.sky((y as f32 / horizon).min(1.0)).mix(pal.bg_bottom, 0.12));
        self.scenery.draw_back(canvas, &view, pal);
        self.draw_land(canvas, horizon, pal);
        self.draw_blur(canvas, horizon);
        self.draw_station(canvas, horizon, pal);
        self.draw_poles(canvas, horizon, pal);
        self.scenery.draw_front(canvas, &view, pal);
        self.draw_window(canvas, pal);
    }

    /// Un click hace pasar un pueblo iluminado.
    fn poke(&mut self, _x: f32, _y: f32) {
        self.town = TOWN_TIME;
        self.town_seed = (self.rng.next() * 1e6) as u32;
    }

    fn new_track(&mut self) {
        if self.fog.is_none() {
            self.fog = Some(0.0);
        }
        self.scenery.fade();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(bands: &Bands, progress: f32) -> Frame<'_> {
        Frame { dt: 1.0 / 60.0, bands, progress: Some(progress), song_len: 120.0, playing: true }
    }

    fn ride(train: &mut Train, bands: &Bands, progress: f32, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            train.update(&frame(bands, progress), 120, 40);
        }
    }

    #[test]
    fn departs_cruises_with_the_tempo_and_stops_at_the_station() {
        let slow = Bands { song: crate::features::Song { bpm: 80.0, tempo_confidence: 0.9, ..Default::default() }, ..Bands::default() };
        let fast = Bands { song: crate::features::Song { bpm: 160.0, tempo_confidence: 0.9, ..Default::default() }, ..Bands::default() };
        let mut train = Train::new(1);
        ride(&mut train, &slow, 0.0, 2.0);
        assert!(train.speed < 0.01, "salió antes de tiempo: {}", train.speed);
        ride(&mut train, &slow, 0.4, 8.0);
        let cruising = train.speed;
        ride(&mut train, &fast, 0.4, 8.0);
        assert!(train.speed > cruising * 1.5, "el tempo no lo apura: {cruising} → {}", train.speed);
        ride(&mut train, &fast, 1.0, 8.0);
        assert!(train.speed < 0.01, "no frenó en la estación: {}", train.speed);
    }

    #[test]
    fn a_pole_crosses_the_middle_on_each_beat() {
        let mut train = Train::new(2);
        let at = |phase: f32| Bands { beat_phase: phase, song: crate::features::Song { bpm: 120.0, tempo_confidence: 0.9, ..Default::default() }, ..Bands::default() };
        train.update(&frame(&at(0.0), 0.5), 120, 40);
        assert_eq!(train.phase(), 0.0, "en el pulso, un poste en el centro");
        train.update(&frame(&at(0.5), 0.5), 120, 40);
        assert_eq!(train.phase(), 0.5);
    }

    #[test]
    fn climax_brings_a_town_and_new_song_restarts_the_trip() {
        let mut train = Train::new(3);
        ride(&mut train, &Bands::default(), 0.6, 3.0);
        train.update(&frame(&Bands { climax: true, ..Bands::default() }, 0.6), 120, 40);
        assert!(train.town > TOWN_TIME - 0.1);
        train.new_track();
        ride(&mut train, &Bands::default(), 0.6, 0.9);
        assert!(train.travel < 0.2, "no volvió a empezar: {}", train.travel);
        ride(&mut train, &Bands::default(), 0.0, 1.0);
        assert!(train.fog.is_none());
    }

    #[test]
    fn draws_at_any_size() {
        let mut train = Train::new(4);
        let bands = Bands { bass: 0.7, level: 0.6, intensity: 0.8, beat: true, climax: true, ..Bands::default() };
        ride(&mut train, &bands, 0.96, 3.0);
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (200, 60)] {
            let mut canvas = Canvas::new(w, h);
            train.draw(&mut canvas, &Palette::default());
        }
    }
}

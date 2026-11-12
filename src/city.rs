//! Escena "ciudad": una ciudad se levanta durante la canción y sus ventanas siguen la música.
//!
//! El cielo y el clima son los del jardín ([`Scenery`]). Los edificios suben de a uno con
//! el avance; cada piso escucha una banda (graves abajo, agudos arriba), las balizas laten
//! con el pulso y los clímax lanzan fuegos artificiales.

use std::f32::consts::TAU;

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::render::View;
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// Alto de la calle, en píxeles.
const STREET: usize = 4;
/// Fracción de la canción que tarda un edificio en levantarse.
const RISE: f32 = 0.07;
/// El último edificio empieza a subir en este punto de la canción.
const BUILT_BY: f32 = 0.55;
const BURST_TIME: f32 = 1.7;

struct Building {
    /// Centro, ancho y alto final, como fracción del ancho y del alto del cielo.
    x: f32,
    width: f32,
    height: f32,
    /// Avance de la canción en que empieza a subir.
    birth: f32,
    /// Capa del fondo: más clara y sin detalle.
    back: bool,
    /// 0 plano, 1 antena, 2 escalonado, 3 aguja.
    roof: u8,
    seed: u32,
}

struct Burst {
    x: f32,
    y: f32,
    age: f32,
    size: f32,
    tint: u8,
    seed: u32,
}

struct Car {
    phase: f32,
    speed: f32,
}

pub struct City {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    /// Reloj del tránsito: avanza con la música.
    traffic: f32,
    progress: f32,
    bands: Bands,
    beat_pulse: f32,
    climax_pulse: f32,
    hat_pulse: f32,
    buildings: Vec<Building>,
    bursts: Vec<Burst>,
    cars: Vec<Car>,
    /// Canción nueva: la ciudad se hunde y se arma otra.
    sinking: Option<f32>,
}

fn hash(a: u32, b: u32, c: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA6B) ^ c.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h >> 8) as f32 / (1 << 24) as f32
}

impl City {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0xC17D_0001);
        scenery.bare = true;
        let mut city = City {
            scenery,
            rng: Rng::new(seed ^ 0x0B1D_5EED),
            time: 0.0,
            traffic: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            beat_pulse: 0.0,
            climax_pulse: 0.0,
            hat_pulse: 0.0,
            buildings: Vec::new(),
            bursts: Vec::new(),
            cars: Vec::new(),
            sinking: None,
        };
        city.generate();
        city
    }

    fn generate(&mut self) {
        let rng = &mut self.rng;
        let mut buildings = Vec::new();
        // Fondo: bloques anchos y bajos que aparecen primero.
        let back = 9;
        for i in 0..back {
            buildings.push(Building {
                x: (i as f32 + 0.5 + rng.range(-0.3, 0.3)) / back as f32,
                width: rng.range(0.08, 0.13),
                height: rng.range(0.22, 0.48),
                birth: rng.range(0.0, 0.22),
                back: true,
                roof: 0,
                seed: (rng.next() * 1e6) as u32,
            });
        }
        // Frente: torres más angostas y altas, las del centro un poco más.
        let front = 8;
        for i in 0..front {
            let x = (i as f32 + 0.5 + rng.range(-0.35, 0.35)) / front as f32;
            let central = 1.0 - (x - 0.5).abs() * 1.2;
            buildings.push(Building {
                x,
                width: rng.range(0.05, 0.085),
                height: (rng.range(0.30, 0.70) * (0.65 + 0.5 * central)).min(0.86),
                birth: rng.range(0.04, BUILT_BY),
                back: false,
                roof: (rng.next() * 4.0) as u8 % 4,
                seed: (rng.next() * 1e6) as u32,
            });
        }
        // Las de atrás se dibujan primero; adelante, las más bajas tapan a las altas.
        buildings.sort_by(|a, b| b.back.cmp(&a.back).then(b.height.total_cmp(&a.height)));
        self.buildings = buildings;
        self.cars = (0..6).map(|_| Car { phase: rng.next(), speed: rng.range(0.05, 0.12) * if rng.next() < 0.5 { -1.0 } else { 1.0 } }).collect();
        self.bursts.clear();
    }

    fn presence(&self) -> f32 {
        self.sinking.map(|t| 1.0 - smoothstep(0.0, 1.0, t)).unwrap_or(1.0)
    }

    /// Cuánto ha subido un edificio, 0..1.
    fn risen(&self, b: &Building) -> f32 {
        let t = ((self.progress - b.birth) / RISE).clamp(0.0, 1.0);
        // Sube rápido y frena al llegar.
        (1.0 - (1.0 - t).powi(3)) * self.presence()
    }

    /// Fracción de la ciudad ya construida.
    #[cfg(test)]
    fn built(&self) -> f32 {
        self.buildings.iter().map(|b| self.risen(b)).sum::<f32>() / self.buildings.len().max(1) as f32
    }

    fn advance(&mut self, f: &Frame) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        if f.playing {
            self.traffic += dt * (0.4 + 1.2 * b.level + (b.song.bpm / 240.0).min(1.0));
        }
        let decay = |v: f32, tau: f32, hit: bool| (v * (-dt / tau).exp()).max(if hit { 1.0 } else { 0.0 });
        self.beat_pulse = decay(self.beat_pulse, 0.28, b.beat);
        self.climax_pulse = decay(self.climax_pulse, 2.5, b.climax);
        self.hat_pulse = decay(self.hat_pulse, 0.15, b.hat);

        if let Some(t) = &mut self.sinking {
            *t += dt / 1.3;
            if *t >= 1.0 {
                self.sinking = None;
                self.progress = 0.0;
                self.generate();
            }
        } else if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }

        // Fuegos artificiales: una tanda en cada clímax, uno suelto en cada cambio de sección.
        let count = if b.climax { 4 } else if b.section { 1 } else { 0 };
        for i in 0..count {
            self.bursts.push(Burst {
                x: self.rng.range(0.12, 0.88),
                y: self.rng.range(0.10, 0.42),
                age: -(i as f32) * self.rng.range(0.15, 0.35),
                size: self.rng.range(0.7, 1.2),
                tint: (self.rng.next() * 3.0) as u8,
                seed: (self.rng.next() * 1e6) as u32,
            });
        }
        for burst in &mut self.bursts {
            burst.age += dt;
        }
        self.bursts.retain(|b| b.age < BURST_TIME);

        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    fn draw_building(&self, canvas: &mut Canvas, b: &Building, ground: f32, pal: &Palette) {
        let risen = self.risen(b);
        if risen < 0.01 {
            return;
        }
        let atm = self.scenery.atmosphere();
        let bands = &self.bands;
        let w = canvas.w as f32;
        let half = ((b.width * w * 0.5).round() as i32).max(2);
        let full = (b.height * ground).round().max(3.0) as i32;
        let tall = (full as f32 * risen).round() as i32;
        let cx = (b.x * w).round() as i32;
        let day = atm.daylight;
        let haze = self.scenery.sky(1.0);
        let wall = Rgb(0.06, 0.07, 0.12).mix(Rgb(0.40, 0.44, 0.54), day * 0.85).mix(pal.stem, 0.10);
        let wall = if b.back { wall.mix(haze, 0.50) } else { wall };
        let night = 1.0 - 0.80 * day;
        let warm = Rgb(1.0, 0.82, 0.46);
        let lit_share = 0.16 + 0.42 * bands.intensity + 0.25 * self.climax_pulse;
        let tick = (self.time * 2.0) as u32;
        let step = if b.back { 3 } else { 2 };

        for j in 0..tall {
            // `floor`: piso contado desde la calle; el edificio sube ya terminado.
            let floor = j;
            let v = floor as f32 / full as f32;
            let y = ground as i32 - 1 - j;
            let heard = if v < 0.34 { bands.bass } else if v < 0.67 { bands.mid } else { bands.treble };
            for i in -half..=half {
                let mut color = if i == -half && !b.back { wall.scale(1.25) } else { wall.scale(1.0 - 0.10 * v) };
                let window = i > -half && i < half && (i + half) % step == 1 && floor % 2 == 1 && floor < full - 1;
                if window {
                    let (col, row) = ((i + half) as u32, floor as u32);
                    let mut lit = hash(b.seed, col, row) < lit_share;
                    // Con los platillos, algunas ventanas cambian de estado.
                    if hash(b.seed ^ tick, col, row) < 0.03 + 0.10 * self.hat_pulse {
                        lit = !lit;
                    }
                    if lit {
                        let tone = if hash(b.seed, row, col) < 0.7 { warm.mix(pal.accent, 0.30) } else { pal.petal_light };
                        let glow = (0.30 + 0.70 * heard + 0.30 * self.beat_pulse) * if b.back { 0.55 } else { 1.0 };
                        color = color.mix(tone, (glow * night).clamp(0.0, 1.0));
                    } else {
                        color = color.scale(0.72);
                    }
                }
                canvas.blend(cx + i, y, color, 1.0);
            }
        }
        if b.back || risen < 0.98 {
            return;
        }

        // Remate de la azotea.
        let top = ground as i32 - tall;
        match b.roof {
            1 => {
                let mast = (full / 5).clamp(2, 7);
                for k in 1..=mast {
                    canvas.blend(cx, top - k, wall.scale(1.3), 1.0);
                }
                // Baliza: late con el pulso.
                let beacon = night * (0.25 + 0.75 * self.beat_pulse);
                for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                    canvas.glow(cx + dx, top - mast - 1 + dy, Rgb(1.0, 0.18, 0.14), beacon * if dx == 0 && dy == 0 { 1.0 } else { 0.35 });
                }
            }
            2 => {
                for k in 1..=3 {
                    for i in -(half - 1).max(1)..=(half - 1).max(1) {
                        canvas.blend(cx + i, top - k, wall.scale(0.92), 1.0);
                    }
                }
            }
            3 => {
                for k in 1..=half.min(6) {
                    for i in -(half - k)..=(half - k) {
                        canvas.blend(cx + i, top - k, wall.scale(1.05), 1.0);
                    }
                }
                canvas.glow(cx, top - half.min(6) - 1, pal.accent, night * (0.3 + 0.7 * bands.treble));
            }
            _ => {}
        }
    }

    fn draw_street(&self, canvas: &mut Canvas, ground: f32, pal: &Palette) {
        let (w, h) = (canvas.w as i32, canvas.px_h() as i32);
        let atm = self.scenery.atmosphere();
        let k = self.presence();
        let asphalt = Rgb(0.04, 0.04, 0.06).mix(Rgb(0.20, 0.21, 0.24), atm.daylight * 0.8);
        // Mojada con la lluvia: refleja un poco el cielo y las ventanas.
        let wet = asphalt.mix(self.scenery.sky(1.0), 0.15 + 0.30 * atm.rain).add(pal.accent, 0.05 * atm.rain * self.bands.level);
        for y in ground as i32..h {
            for x in 0..w {
                let dash = y == ground as i32 + (STREET as i32) / 2 && (x + (self.traffic * 3.0) as i32).rem_euclid(8) < 3;
                canvas.blend(x, y, if dash { wet.add(Rgb::WHITE, 0.10) } else { wet }, 1.0);
            }
        }
        // Autos: un faro blanco adelante y una luz roja atrás.
        for (n, car) in self.cars.iter().enumerate() {
            let x = ((car.phase + self.traffic * car.speed).rem_euclid(1.0) * (w as f32 + 8.0)) as i32 - 4;
            let y = ground as i32 + if car.speed > 0.0 { 1 } else { (STREET as i32 - 1).max(1) };
            let dir = if car.speed > 0.0 { 1 } else { -1 };
            let body = [pal.petal, pal.accent, pal.leaf][n % 3].scale(0.55);
            for i in 0..3 {
                canvas.blend(x - dir * i, y, body, k);
            }
            canvas.blend(x + dir, y, Rgb(1.0, 0.96, 0.80), k);
            canvas.glow(x + dir * 2, y, Rgb(1.0, 0.96, 0.80), 0.35 * k * (1.0 - 0.7 * atm.daylight));
            canvas.blend(x - dir * 3, y, Rgb(0.95, 0.12, 0.10), k * 0.9);
        }
    }

    fn draw_bursts(&self, canvas: &mut Canvas, ground: f32, pal: &Palette) {
        let w = canvas.w as f32;
        for burst in self.bursts.iter().filter(|b| b.age >= 0.0) {
            let t = burst.age / BURST_TIME;
            let color = [pal.petal_light, pal.accent, pal.petal][burst.tint as usize % 3].mix(Rgb::WHITE, 0.25);
            let (cx, cy) = (burst.x * w, burst.y * ground);
            let reach = ground * 0.26 * burst.size;
            let fade = (1.0 - t).powf(1.3);
            // Destello al estallar.
            if t < 0.15 {
                let flash = 1.0 - t / 0.15;
                for dy in -4i32..=4 {
                    for dx in -4i32..=4 {
                        canvas.glow(cx as i32 + dx, cy as i32 + dy, color, 0.5 * flash * (-((dx * dx + dy * dy) as f32) / 6.0).exp());
                    }
                }
            }
            let rays = 18;
            for ray in 0..rays {
                let angle = ray as f32 / rays as f32 * TAU + hash(burst.seed, ray, 0) * 0.3;
                let power = 0.65 + 0.35 * hash(burst.seed, ray, 1);
                // Cada chispa deja una cola corta y cae con la gravedad.
                for tail in 0..5 {
                    let u = (t - tail as f32 * 0.025).max(0.0);
                    let r = reach * power * (1.0 - (-4.0 * u).exp());
                    let (x, y) = (cx + angle.cos() * r, cy + angle.sin() * r + reach * 0.55 * u * u);
                    if y < ground - 1.0 {
                        canvas.dot_over((x * 2.0) as i32, (y * 2.0) as i32, color, fade * (1.0 - tail as f32 * 0.18));
                    }
                }
            }
        }
    }
}

impl Scene for City {
    fn update(&mut self, frame: &Frame, _w: usize, _h: usize) {
        self.advance(frame);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let ph = canvas.px_h();
        let ground = ph.saturating_sub(STREET).max(1) as f32;
        let view = View { cx: canvas.w as f32 / 2.0, ground, scale: ground.max(4.0), weight: 1.0 };
        let lift = 0.022 * self.bands.intensity + 0.09 * self.climax_pulse;
        let scenery = &self.scenery;
        canvas.clear(|_, y| scenery.sky((y as f32 / ground).min(1.0)).mix(pal.bg_bottom, 0.12).add(pal.petal, lift));

        self.scenery.draw_back(canvas, &view, pal);
        self.draw_bursts(canvas, ground, pal);
        for b in &self.buildings {
            self.draw_building(canvas, b, ground, pal);
        }
        self.draw_street(canvas, ground, pal);
        self.scenery.draw_front(canvas, &view, pal);
    }

    fn new_track(&mut self) {
        if self.sinking.is_none() {
            self.sinking = Some(0.0);
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

    #[test]
    fn city_rises_with_the_song() {
        let mut city = City::new(1);
        let bands = Bands::default();
        city.update(&frame(&bands, 0.0), 120, 40);
        let start = city.built();
        city.update(&frame(&bands, 0.3), 120, 40);
        let middle = city.built();
        city.update(&frame(&bands, BUILT_BY + RISE), 120, 40);
        let end = city.built();
        assert!(start < 0.05, "empezó construida: {start}");
        assert!(start < middle && middle < end, "{start} {middle} {end}");
        assert!(end > 0.999, "no terminó: {end}");
    }

    #[test]
    fn climax_launches_fireworks() {
        let mut city = City::new(2);
        let bands = Bands { climax: true, ..Bands::default() };
        city.update(&frame(&bands, 0.5), 120, 40);
        assert_eq!(city.bursts.len(), 4);
        let quiet = Bands::default();
        for _ in 0..(60.0 * (BURST_TIME + 1.5)) as usize {
            city.update(&frame(&quiet, 0.5), 120, 40);
        }
        assert!(city.bursts.is_empty());
    }

    #[test]
    fn draws_at_any_size() {
        let mut city = City::new(3);
        let bands = Bands { bass: 0.7, treble: 0.6, level: 0.6, intensity: 0.8, climax: true, beat: true, hat: true, ..Bands::default() };
        for _ in 0..30 {
            city.update(&frame(&bands, 0.9), 80, 24);
        }
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (200, 60)] {
            let mut canvas = Canvas::new(w, h);
            city.draw(&mut canvas, &Palette::default());
        }
    }

    #[test]
    fn new_song_builds_another_city() {
        let mut city = City::new(4);
        let bands = Bands::default();
        city.update(&frame(&bands, 0.9), 120, 40);
        let before: Vec<f32> = city.buildings.iter().map(|b| b.height).collect();
        city.new_track();
        for _ in 0..100 {
            city.update(&frame(&bands, 0.0), 120, 40);
        }
        let after: Vec<f32> = city.buildings.iter().map(|b| b.height).collect();
        assert!(city.sinking.is_none() && city.built() < 0.05);
        assert_ne!(before, after);
    }
}

//! Escena "estanque": un estanque de kois visto desde arriba.
//!
//! Los kois nadan dando vueltas: cada uno es una cadena de segmentos que sigue a la
//! cabeza, así el cuerpo ondula al doblar. Hay nenúfares con flores de loto que se abren
//! durante la canción, pétalos que flotan y piedras en la orilla. El tempo pone la
//! velocidad de los peces, cada golpe fuerte hace una onda en el agua y en el clímax los
//! peces se apuran todos. De noche el agua se oscurece y refleja la luna. Un click hace una
//! onda y los peces van hacia ahí, como si les tiraran comida.

use std::f32::consts::{PI, TAU};

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

const FISH: usize = 7;
const SEGMENTS: usize = 12;
const MAX_RINGS: usize = 30;
const MAX_PETALS: usize = 60;

/// Diseños de koi.
#[derive(Clone, Copy, Debug)]
enum Breed {
    Kohaku,
    Sanke,
    Showa,
    Ogon,
    Chagoi,
    Asagi,
}

struct Fish {
    /// Cabeza (0) y el resto del cuerpo, en unidades del alto: y de 0 a 1, x de 0 a `aspect`.
    body: [(f32, f32); SEGMENTS],
    heading: f32,
    turn: f32,
    /// Largo del cuerpo.
    size: f32,
    breed: Breed,
    /// Los hondos se ven más apagados y van debajo de los nenúfares.
    depth: f32,
    seed: u32,
    /// Arranque rápido de un golpe, que se apaga solo.
    dart: f32,
}

struct Pad {
    x: f32,
    y: f32,
    r: f32,
    notch: f32,
    /// Flor de loto encima: cuándo se abre (avance de la canción) y su color.
    lotus: Option<(f32, u8)>,
}

struct Ring {
    x: f32,
    y: f32,
    age: f32,
    strength: f32,
}

struct Petal {
    x: f32,
    y: f32,
    spin: f32,
    color: u8,
}

pub struct Pond {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    progress: f32,
    bands: Bands,
    /// Ancho relativo al alto (en píxeles), para que los círculos sean círculos.
    aspect: f32,
    fish: Vec<Fish>,
    pads: Vec<Pad>,
    rings: Vec<Ring>,
    petals: Vec<Petal>,
    /// Comida: los peces nadan hacia ahí un rato.
    food: Option<(f32, f32, f32)>,
    climax: f32,
    seed: u32,
}

fn hash(a: u32, b: u32) -> f32 {
    let h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    let h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
    (h ^ (h >> 13)) as f32 / u32::MAX as f32
}

/// Ángulo en (−π, π].
fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

impl Pond {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0x0C01_7001);
        scenery.bare = true;
        let mut pond = Pond {
            scenery,
            rng: Rng::new(seed ^ 0x0C01_C01F),
            time: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            aspect: 2.0,
            fish: Vec::new(),
            pads: Vec::new(),
            rings: Vec::new(),
            petals: Vec::new(),
            food: None,
            climax: 0.0,
            seed: 0,
        };
        pond.stock();
        pond
    }

    /// Llena el estanque: peces de distintas razas y nenúfares repartidos.
    fn stock(&mut self) {
        self.seed = (self.rng.next() * 1e6) as u32;
        let a = self.aspect;
        let breeds = [Breed::Kohaku, Breed::Sanke, Breed::Showa, Breed::Ogon, Breed::Chagoi, Breed::Asagi];
        self.fish = (0..FISH)
            .map(|i| {
                let (x, y) = (self.rng.range(0.2, a - 0.2), self.rng.range(0.2, 0.8));
                Fish {
                    body: [(x, y); SEGMENTS],
                    heading: self.rng.range(0.0, TAU),
                    turn: 0.0,
                    size: self.rng.range(0.13, 0.22),
                    breed: breeds[(i + (self.rng.next() * 6.0) as usize) % breeds.len()],
                    depth: self.rng.range(0.0, 1.0),
                    seed: (self.rng.next() * 1e6) as u32,
                    dart: 0.0,
                }
            })
            .collect();
        self.pads = (0..9)
            .map(|i| Pad {
                x: self.rng.range(0.1, a - 0.1),
                y: self.rng.range(0.1, 0.9),
                r: self.rng.range(0.04, 0.08),
                notch: self.rng.range(0.0, TAU),
                lotus: (i < 3 || self.rng.next() < 0.3).then(|| (self.rng.range(0.1, 0.6), (self.rng.next() * 3.0) as u8)),
            })
            .collect();
    }

    fn advance(&mut self, f: &Frame, w: usize, h: usize) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        if w > 0 && h > 0 {
            let aspect = w as f32 / (h * 2) as f32;
            if (aspect - self.aspect).abs() > 0.01 {
                // Al cambiar de tamaño, todo se estira al ancho nuevo.
                let k = aspect / self.aspect;
                for fish in &mut self.fish {
                    for p in &mut fish.body {
                        p.0 *= k;
                    }
                }
                for pad in &mut self.pads {
                    pad.x *= k;
                }
                self.aspect = aspect;
            }
        }
        if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }
        self.climax = (self.climax * (-dt / 3.0).exp()).max(if b.climax { 1.0 } else { 0.0 });
        if let Some((_, _, t)) = &mut self.food {
            *t -= dt;
            if *t <= 0.0 {
                self.food = None;
            }
        }

        // Ondas: los golpes fuertes, la lluvia y los peces que suben a la superficie.
        let rain = self.scenery.atmosphere().rain;
        let mut drops = if b.kick && b.bass > 0.4 && f.playing { 1 } else { 0 };
        if self.rng.next() < dt * (0.3 + 12.0 * rain) {
            drops += 1;
        }
        for _ in 0..drops {
            if self.rings.len() < MAX_RINGS {
                let (x, y) = (self.rng.range(0.05, self.aspect - 0.05), self.rng.range(0.05, 0.95));
                self.rings.push(Ring { x, y, age: 0.0, strength: 0.6 + 0.4 * b.bass });
            }
        }
        for r in &mut self.rings {
            r.age += dt;
        }
        self.rings.retain(|r| r.age < 2.2);

        // Peces: deambulan, esquivan la orilla y, si hay comida, van hacia ella.
        let tempo = if b.song.bpm > 0.0 { (b.song.bpm / 110.0).clamp(0.6, 1.6) } else { 1.0 };
        let base = (0.05 + 0.06 * b.level + 0.08 * self.climax) * tempo * if f.playing { 1.0 } else { 0.4 };
        let aspect = self.aspect;
        let food = self.food;
        for fish in &mut self.fish {
            if b.beat && b.intensity > 0.5 && self.rng.next() < 0.3 {
                fish.dart = 1.0;
            }
            fish.dart *= (-dt / 0.6).exp();
            fish.turn += self.rng.range(-1.0, 1.0) * dt * 2.0;
            fish.turn = fish.turn.clamp(-1.2, 1.2) * (-dt * 0.5).exp();
            let (x, y) = fish.body[0];
            // Hacia el centro si se acerca a la orilla.
            let margin = 0.12;
            let mut want = None;
            if x < margin || x > aspect - margin || y < margin || y > 1.0 - margin {
                want = Some((aspect / 2.0 - x).atan2(0.5 - y));
            }
            if let Some((fx, fy, _)) = food {
                want = Some((fx - x).atan2(fy - y));
            }
            if let Some(target) = want {
                let diff = wrap(target - fish.heading);
                fish.turn += diff.clamp(-1.0, 1.0) * dt * 4.0;
            }
            fish.heading = wrap(fish.heading + fish.turn * dt);
            let speed = base * (0.7 + 0.6 * fish.size / 0.2) * (1.0 + 1.5 * fish.dart);
            let (sx, sy) = fish.heading.sin_cos();
            fish.body[0] = ((x + sx * speed * dt).clamp(0.0, aspect), (y + sy * speed * dt).clamp(0.0, 1.0));
            // El resto del cuerpo sigue a la cabeza a distancia fija: así ondula al doblar.
            let gap = fish.size / (SEGMENTS - 1) as f32;
            for k in 1..SEGMENTS {
                let (px, py) = fish.body[k - 1];
                let (cx, cy) = fish.body[k];
                let (dx, dy) = (cx - px, cy - py);
                let d = (dx * dx + dy * dy).sqrt().max(1e-5);
                fish.body[k] = (px + dx / d * gap, py + dy / d * gap);
            }
        }

        // Pétalos que caen al agua y flotan a la deriva.
        if self.petals.len() < MAX_PETALS && self.rng.next() < dt * (0.3 + 1.5 * b.treble) {
            self.petals.push(Petal { x: self.rng.range(0.0, aspect), y: self.rng.range(0.0, 1.0), spin: self.rng.range(0.0, TAU), color: (self.rng.next() * 3.0) as u8 });
        }
        let wind = self.scenery.atmosphere().wind;
        for p in &mut self.petals {
            p.spin += dt * 0.5;
            p.x += dt * (0.01 + 0.02 * wind) * p.spin.cos();
            p.y += dt * 0.008 * p.spin.sin();
        }
        self.petals.retain(|p| p.x > -0.02 && p.x < aspect + 0.02);
        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    /// Color del koi en un punto del cuerpo: `t` de la cabeza (0) a la cola (1), `side` de −1
    /// a 1 a lo ancho.
    fn fish_color(fish: &Fish, t: f32, side: f32) -> Rgb {
        let white = Rgb(0.96, 0.95, 0.92);
        let red = Rgb(0.95, 0.28, 0.12);
        let black = Rgb(0.06, 0.06, 0.07);
        let patch = |k: u32| hash(fish.seed ^ k, ((t * 5.0) as u32) ^ ((side * 1.5 + 2.0) as u32)) ;
        match fish.breed {
            Breed::Kohaku => if patch(1) < 0.55 { red } else { white },
            Breed::Sanke => if patch(2) < 0.18 { black } else if patch(1) < 0.5 { red } else { white },
            Breed::Showa => if patch(2) < 0.45 { black } else if patch(1) < 0.55 { red } else { white },
            Breed::Ogon => Rgb(1.0, 0.78, 0.25).mix(Rgb(1.0, 0.95, 0.6), 0.3 * (1.0 - side.abs())),
            Breed::Chagoi => Rgb(0.55, 0.38, 0.20),
            Breed::Asagi => if side.abs() > 0.5 { Rgb(0.85, 0.40, 0.25) } else { Rgb(0.45, 0.58, 0.72) },
        }
    }

    fn draw_fish(&self, canvas: &mut Canvas, fish: &Fish, scale: f32, day: f32) {
        let dim = (0.55 + 0.45 * (1.0 - fish.depth)) * (0.45 + 0.55 * day.max(0.35));
        let water = Rgb(0.05, 0.18, 0.20);
        // Aletas: dos manchas a los lados, cerca de la cabeza, y la cola en abanico.
        let (hx, hy) = fish.body[2];
        let (nx, ny) = fish.body[1];
        let (ax, ay) = (hx - nx, hy - ny);
        let len = (ax * ax + ay * ay).sqrt().max(1e-5);
        let (px, py) = (-ay / len, ax / len);
        let fin = fish.size * 0.09;
        let flap = (self.time * 6.0 + fish.seed as f32).sin() * 0.3;
        for s in [-1.0f32, 1.0] {
            let (fx, fy) = (hx + px * s * fish.size * (0.12 + 0.03 * flap), hy + py * s * fish.size * (0.12 + 0.03 * flap));
            Self::blob(canvas, fx, fy, fin * 0.7, scale, Rgb(0.95, 0.9, 0.85).mix(water, 0.45).scale(dim), 0.6);
        }
        for k in 0..SEGMENTS {
            let t = k as f32 / (SEGMENTS - 1) as f32;
            let (x, y) = fish.body[k];
            // Grueso detrás de la cabeza, fino en la cola.
            let r = fish.size * 0.10 * ((t * 0.92 + 0.10) * PI).sin().powf(0.8).max(0.25);
            let c = Self::fish_color(fish, t, 0.0).mix(water, 0.25 * fish.depth).scale(dim);
            Self::blob(canvas, x, y, r, scale, c, 1.0);
        }
        // Manchas a los lados para que el dibujo no sea una sola franja.
        for k in (1..SEGMENTS - 2).step_by(2) {
            let t = k as f32 / (SEGMENTS - 1) as f32;
            let (x, y) = fish.body[k];
            let (x2, y2) = fish.body[k + 1];
            let (dx, dy) = (x2 - x, y2 - y);
            let l = (dx * dx + dy * dy).sqrt().max(1e-5);
            let r = fish.size * 0.06;
            for s in [-1.0f32, 1.0] {
                let c = Self::fish_color(fish, t, s).mix(water, 0.25 * fish.depth).scale(dim);
                Self::blob(canvas, x - dy / l * s * r * 0.8, y + dx / l * s * r * 0.8, r * 0.7, scale, c, 1.0);
            }
        }
        // Cola: un abanico que se mueve.
        let (tx, ty) = fish.body[SEGMENTS - 1];
        let (bx, by) = fish.body[SEGMENTS - 2];
        let (dx, dy) = (tx - bx, ty - by);
        let l = (dx * dx + dy * dy).sqrt().max(1e-5);
        let swish = (self.time * 8.0 * (1.0 + fish.dart) + fish.seed as f32).sin() * 0.5;
        for s in [-1.0f32, 1.0] {
            let ang = (dy / l).atan2(dx / l) + s * 0.5 + swish;
            let (cx, cy) = (tx + ang.cos() * fish.size * 0.07, ty + ang.sin() * fish.size * 0.07);
            Self::blob(canvas, cx, cy, fish.size * 0.05, scale, Rgb(0.95, 0.9, 0.85).mix(water, 0.4).scale(dim), 0.7);
        }
    }

    /// Un círculo relleno en coordenadas de alto (`scale` = píxeles por unidad).
    fn blob(canvas: &mut Canvas, x: f32, y: f32, r: f32, scale: f32, c: Rgb, alpha: f32) {
        let (cx, cy, rp) = (x * scale, y * scale, (r * scale).max(0.5));
        for py in (cy - rp) as i32..=(cy + rp) as i32 {
            for px in (cx - rp) as i32..=(cx + rp) as i32 {
                let d = ((px as f32 + 0.5 - cx).powi(2) + (py as f32 + 0.5 - cy).powi(2)).sqrt();
                if d <= rp + 0.2 {
                    canvas.paint(px, py, c, alpha);
                }
            }
        }
    }

    fn draw_pad(&self, canvas: &mut Canvas, pad: &Pad, scale: f32, day: f32, pal: &Palette) {
        let leaf = Rgb(0.20, 0.50, 0.22).mix(pal.leaf, 0.2).scale(0.45 + 0.55 * day.max(0.3));
        let (cx, cy, r) = (pad.x * scale, pad.y * scale, pad.r * scale);
        let sway = 0.05 * (self.time * 0.4 + pad.notch).sin();
        for py in (cy - r) as i32..=(cy + r) as i32 {
            for px in (cx - r) as i32..=(cx + r) as i32 {
                let (dx, dy) = (px as f32 + 0.5 - cx, py as f32 + 0.5 - cy);
                let d = (dx * dx + dy * dy).sqrt();
                if d > r {
                    continue;
                }
                let a = dy.atan2(dx);
                if wrap(a - pad.notch - sway).abs() < 0.28 {
                    continue;
                }
                // Nervaduras desde el centro y un borde más oscuro.
                let vein = ((a * 4.0).sin().abs() < 0.12) as i32 as f32;
                let c = leaf.scale(if d > r - 1.0 { 0.75 } else { 1.0 - 0.15 * vein }).add(Rgb::WHITE, 0.05 * (1.0 - d / r));
                canvas.solid(px, py, c);
            }
        }
        // Flor de loto que se abre durante la canción.
        if let Some((at, color)) = pad.lotus {
            let open = (smoothstep(at, at + 0.25, self.progress) + self.climax * 0.5).min(1.0);
            if open > 0.02 {
                let petal = [Rgb(1.0, 0.70, 0.82), Rgb(1.0, 0.95, 0.95), pal.petal][color as usize % 3].scale(0.5 + 0.5 * day.max(0.4));
                let (fx, fy) = (cx + r * 0.15, cy - r * 0.1);
                let pr = (r * 0.45 * (0.4 + 0.6 * open)).max(1.0);
                for k in 0..8 {
                    let a = k as f32 / 8.0 * TAU + pad.notch;
                    let (ex, ey) = (fx + a.cos() * pr * 0.7, fy + a.sin() * pr * 0.7);
                    for py in (ey - pr * 0.45) as i32..=(ey + pr * 0.45) as i32 {
                        for px in (ex - pr * 0.45) as i32..=(ex + pr * 0.45) as i32 {
                            let d = ((px as f32 + 0.5 - ex).powi(2) + (py as f32 + 0.5 - ey).powi(2)).sqrt();
                            if d <= pr * 0.45 {
                                canvas.solid(px, py, petal.scale(0.85 + 0.15 * (k % 2) as f32));
                            }
                        }
                    }
                }
                canvas.solid(fx as i32, fy as i32, Rgb(1.0, 0.85, 0.3));
            }
        }
    }
}

impl Scene for Pond {
    fn update(&mut self, frame: &Frame, w: usize, h: usize) {
        self.advance(frame, w, h);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        if w < 2.0 || h < 2.0 {
            return;
        }
        let scale = h;
        let atm = self.scenery.atmosphere();
        let day = atm.daylight;
        // Agua: verde azulado, con el cielo reflejado y la luz que ondula en el fondo.
        let deep = Rgb(0.03, 0.14, 0.16).mix(Rgb(0.01, 0.03, 0.06), 1.0 - day);
        let sky = self.scenery.sky(0.3);
        let (mx, my) = (w * 0.72, h * 0.28);
        let light = if day > 0.4 { Rgb(1.0, 0.95, 0.8) } else { Rgb(0.85, 0.88, 1.0) };
        let shine = if day > 0.4 { atm.sun } else { atm.moon };
        let t = self.time;
        canvas.clear(|x, y| {
            let (u, v) = (x as f32 / h, y as f32 / h);
            let caustic = ((u * 13.0 + t * 0.6).sin() * (v * 11.0 - t * 0.45).sin() + (u * 7.0 - v * 5.0 + t * 0.3).sin() * 0.5).max(0.0);
            let mut c = deep.mix(sky, 0.22).add(Rgb(0.4, 0.8, 0.7), 0.05 * caustic * (0.3 + 0.7 * day));
            // Reflejo del sol o la luna, con un temblor.
            let d = ((x as f32 - mx + (t * 2.0 + y as f32 * 0.5).sin()).powi(2) + (y as f32 - my).powi(2)).sqrt() / (h * 0.10);
            c = c.add(light, 0.35 * shine * (1.0 - d).max(0.0).powi(2));
            c
        });
        // Ondas.
        for r in &self.rings {
            let (cx, cy) = (r.x * scale, r.y * scale);
            let rad = r.age * 0.12 * scale;
            let fade = (1.0 - r.age / 2.2).powi(2) * r.strength;
            let steps = (rad * 6.0).max(8.0) as i32;
            for k in 0..steps {
                let a = k as f32 / steps as f32 * TAU;
                canvas.paint((cx + a.cos() * rad) as i32, (cy + a.sin() * rad) as i32, Rgb(0.8, 0.95, 1.0), 0.35 * fade);
            }
        }
        // Peces hondos, nenúfares, y encima los que nadan cerca de la superficie.
        for fish in self.fish.iter().filter(|f| f.depth > 0.5) {
            self.draw_fish(canvas, fish, scale, day);
        }
        for pad in &self.pads {
            self.draw_pad(canvas, pad, scale, day, pal);
        }
        for fish in self.fish.iter().filter(|f| f.depth <= 0.5) {
            self.draw_fish(canvas, fish, scale, day);
        }
        // Pétalos flotando.
        for p in &self.petals {
            let c = [pal.petal, Rgb(1.0, 0.85, 0.9), Rgb(1.0, 0.95, 0.85)][p.color as usize % 3].scale(0.5 + 0.5 * day.max(0.4));
            let (x, y) = ((p.x * scale) as i32, (p.y * scale) as i32);
            canvas.solid(x, y, c);
            if p.spin.sin() > 0.0 {
                canvas.solid(x + 1, y, c.scale(0.85));
            }
        }
        // Piedras de la orilla, abajo y en una esquina.
        for i in 0..((w / 4.0) as u32 + 4) {
            let along = hash(self.seed, i);
            let (x, y) = if i % 3 == 0 { (along * w * 0.35, h - 1.0 - hash(self.seed, i + 99) * 2.0) } else { (w - hash(self.seed, i + 7) * w * 0.12, along * h) };
            let r = 1.5 + hash(self.seed, i + 50) * (h * 0.05);
            let stone = Rgb(0.42, 0.40, 0.38).scale(0.5 + 0.4 * hash(self.seed, i + 3)).scale(0.45 + 0.55 * day.max(0.3));
            for py in (y - r) as i32..=(y + r) as i32 {
                for px in (x - r * 1.2) as i32..=(x + r * 1.2) as i32 {
                    let d = (((px as f32 - x) / 1.2).powi(2) + (py as f32 - y).powi(2)).sqrt();
                    if d <= r {
                        let lit = 1.0 + 0.25 * (((y - py as f32) - (px as f32 - x) * 0.3) / r).clamp(-1.0, 1.0);
                        let moss = hash(px as u32 ^ self.seed, py as u32) < 0.15;
                        canvas.solid(px, py, if moss { Rgb(0.2, 0.35, 0.15).scale(0.5 + 0.5 * day.max(0.3)) } else { stone.scale(lit) });
                    }
                }
            }
        }
        // Luciérnagas sobre el agua, de noche.
        if day < 0.4 {
            for i in 0..8u32 {
                let x = (hash(self.seed, i + 200) * w + (t * 0.3 + i as f32).sin() * 3.0) as i32;
                let y = (hash(self.seed, i + 300) * h + (t * 0.4 + i as f32).cos() * 2.0) as i32;
                let on = ((t * 1.3 + i as f32 * 2.1).sin() * 0.5 + 0.5).powi(3);
                canvas.glow(x, y, Rgb(0.85, 1.0, 0.45), 0.7 * on * (1.0 - day / 0.4));
            }
        }
    }

    fn new_track(&mut self) {
        // La transición tapa el cambio: el estanque nuevo trae otros peces y nenúfares.
        self.stock();
        self.rings.clear();
        self.petals.clear();
        self.food = None;
        self.scenery.fade();
    }

    /// Un click: una onda ahí y los peces se acercan, como si les tiraran comida.
    fn poke(&mut self, x: f32, y: f32) {
        let (fx, fy) = (x * self.aspect, y);
        self.food = Some((fx, fy, 4.0));
        for k in 0..2 {
            if self.rings.len() < MAX_RINGS + 4 {
                self.rings.push(Ring { x: fx, y: fy, age: -0.25 * k as f32, strength: 1.0 });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(p: &mut Pond, bands: &Bands, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            p.update(&Frame { dt: 1.0 / 60.0, bands, progress: Some(0.5), song_len: 120.0, playing: true }, 120, 30);
        }
    }

    #[test]
    fn fish_stay_in_the_pond_and_keep_their_length() {
        let mut p = Pond::new(1);
        run(&mut p, &Bands { level: 1.0, ..Bands::default() }, 30.0);
        for fish in &p.fish {
            for &(x, y) in &fish.body {
                assert!((-0.05..=p.aspect + 0.05).contains(&x) && (-0.05..=1.05).contains(&y), "{x},{y}");
            }
            let (a, b) = (fish.body[0], fish.body[SEGMENTS - 1]);
            let span = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
            assert!(span <= fish.size * 1.01, "{span} > {}", fish.size);
        }
    }

    #[test]
    fn a_click_feeds_the_fish() {
        let mut p = Pond::new(2);
        run(&mut p, &Bands::default(), 1.0);
        let target = (p.aspect * 0.5, 0.5);
        let dist = |p: &Pond| p.fish.iter().map(|f| ((f.body[0].0 - target.0).powi(2) + (f.body[0].1 - target.1).powi(2)).sqrt()).sum::<f32>();
        let before = dist(&p);
        p.poke(0.5, 0.5);
        run(&mut p, &Bands::default(), 3.0);
        assert!(dist(&p) < before, "{} !< {before}", dist(&p));
        assert!(!p.rings.is_empty());
    }

    #[test]
    fn loud_kicks_make_ripples_and_lotus_opens_with_the_song() {
        let mut p = Pond::new(3);
        p.rings.clear();
        run(&mut p, &Bands { kick: true, bass: 0.9, ..Bands::default() }, 0.2);
        assert!(!p.rings.is_empty());
        p.progress = 0.0;
        let pad = p.pads.iter().find(|pad| pad.lotus.is_some());
        assert!(pad.is_some());
    }

    #[test]
    fn draws_at_any_size() {
        let mut p = Pond::new(4);
        let bands = Bands { level: 1.0, bass: 1.0, treble: 1.0, kick: true, beat: true, climax: true, intensity: 1.0, ..Bands::default() };
        run(&mut p, &bands, 1.0);
        p.poke(0.3, 0.3);
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (220, 60)] {
            let mut canvas = Canvas::new(w, h);
            p.draw(&mut canvas, &Palette::default());
        }
    }
}

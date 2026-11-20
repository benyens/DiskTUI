//! Escena "bosque": un bosque frondoso que crece y florece durante la canción.
//!
//! Tres capas de árboles: los del fondo, azulados por la distancia, ya están; los del medio
//! y los del frente nacen como brotes y van creciendo. A mitad de su vida, los que florecen
//! se llenan de flores rosadas, lilas, amarillas, rojas o blancas. Los graves mecen las
//! copas, los golpes sueltan pétalos y el clímax hace florecer todo de una vez. De día entran
//! rayos de luz entre las copas; de noche hay luciérnagas. Cada canción trae un bosque
//! distinto.

use std::f32::consts::TAU;

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::render::View;
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// Suelo de cada capa (fondo, medio, frente), como fracción del alto.
const GROUND: [f32; 3] = [0.70, 0.78, 0.90];
const MAX_PETALS: usize = 400;
const MAX_FLIES: usize = 40;

struct Tree {
    x: f32,
    layer: usize,
    /// Alto final, como fracción del alto del lienzo.
    height: f32,
    /// Copas: (desplazamiento x, altura sobre el tronco, radio), en altos del árbol.
    blobs: Vec<(f32, f32, f32)>,
    /// 0 = solo hojas; si no, el color de sus flores (índice en `flower_color`).
    species: u8,
    /// Avance de la canción en que brota.
    birth: f32,
    lean: f32,
}

struct Petal {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    color: u8,
    spin: f32,
}

struct Firefly {
    x: f32,
    y: f32,
    phase: f32,
}

pub struct Forest {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    progress: f32,
    bands: Bands,
    trees: Vec<Tree>,
    petals: Vec<Petal>,
    flies: Vec<Firefly>,
    /// Floración extra del clímax o de un click, que se apaga sola.
    burst: f32,
    sway: f32,
    seed: u32,
}

fn hash(a: u32, b: u32) -> f32 {
    let h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    let h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
    (h ^ (h >> 13)) as f32 / u32::MAX as f32
}

fn flower_color(species: u8, pal: &Palette) -> Rgb {
    match species {
        1 => Rgb(1.0, 0.68, 0.82),  // cerezo
        2 => Rgb(0.70, 0.52, 1.0),  // jacarandá
        3 => Rgb(1.0, 0.86, 0.25),  // aromo
        4 => Rgb(1.0, 0.35, 0.25),  // ceibo
        5 => Rgb(1.0, 0.97, 0.92),  // blanco
        _ => pal.petal,             // del color de la carátula
    }
}

impl Forest {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0xF0E5_7001);
        scenery.bare = true;
        let mut forest = Forest {
            scenery,
            rng: Rng::new(seed ^ 0x0B05_C0E5),
            time: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            trees: Vec::new(),
            petals: Vec::new(),
            flies: Vec::new(),
            burst: 0.0,
            sway: 0.0,
            seed: 0,
        };
        forest.plant();
        forest
    }

    /// Siembra un bosque nuevo: cada capa con sus árboles repartidos y algo desordenados.
    fn plant(&mut self) {
        self.seed = (self.rng.next() * 1e6) as u32;
        self.trees.clear();
        for (layer, (count, size, births)) in [(13, 0.30, (-0.6, -0.35)), (9, 0.48, (-0.05, 0.22)), (5, 0.72, (0.06, 0.40))].into_iter().enumerate() {
            for i in 0..count {
                let x = (i as f32 + self.rng.range(0.1, 0.9)) / count as f32 * 1.1 - 0.05;
                let height = size * self.rng.range(0.8, 1.15);
                let mut blobs = Vec::new();
                let n = 4 + (self.rng.next() * 4.0) as usize;
                for _ in 0..n {
                    let a = self.rng.range(0.0, TAU);
                    let spread = self.rng.range(0.0, 0.22);
                    blobs.push((a.cos() * spread, 0.62 + a.sin().abs() * spread * 1.2 + self.rng.range(-0.05, 0.12), self.rng.range(0.14, 0.24)));
                }
                // Un poco más de la mitad florece; el resto es puro verde.
                let species = if self.rng.next() < 0.6 { 1 + (self.rng.next() * 6.0) as u8 % 6 } else { 0 };
                self.trees.push(Tree {
                    x,
                    layer,
                    height,
                    blobs,
                    species,
                    birth: self.rng.range(births.0, births.1),
                    lean: self.rng.range(-0.06, 0.06),
                });
            }
        }
    }

    /// Cuánto creció un árbol (0 a 1) y cuánto florece.
    fn growth(&self, t: &Tree) -> (f32, f32) {
        let g = smoothstep(t.birth, t.birth + 0.30, self.progress);
        let bloom = if t.species == 0 { 0.0 } else { (smoothstep(t.birth + 0.22, t.birth + 0.50, self.progress) + self.burst).min(1.0) };
        (g, bloom)
    }

    fn advance(&mut self, f: &Frame) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }
        self.burst = (self.burst * (-dt / 3.0).exp()).max(if b.climax { 1.0 } else { 0.0 });
        self.sway += dt * (0.8 + 1.5 * b.bass);

        // Los golpes sueltan pétalos desde los árboles en flor.
        let release = if b.beat { 3 + (b.intensity * 6.0) as usize } else { 0 } + if b.climax { 40 } else { 0 };
        let blooming: Vec<(f32, f32, u8)> = self
            .trees
            .iter()
            .filter_map(|t| {
                let (g, bloom) = self.growth(t);
                (bloom > 0.3).then_some((t.x, GROUND[t.layer] - t.height * g * 0.7, t.species))
            })
            .collect();
        if !blooming.is_empty() && f.playing {
            for _ in 0..release.min(MAX_PETALS.saturating_sub(self.petals.len())) {
                let (x, y, species) = blooming[(self.rng.next() * blooming.len() as f32) as usize % blooming.len()];
                self.petals.push(Petal {
                    x: x + self.rng.range(-0.04, 0.04),
                    y: y + self.rng.range(-0.06, 0.06),
                    vx: self.rng.range(-0.02, 0.05),
                    vy: self.rng.range(0.02, 0.06),
                    color: species,
                    spin: self.rng.range(0.0, TAU),
                });
            }
        }
        let wind = self.scenery.atmosphere().wind;
        for p in &mut self.petals {
            p.spin += dt * 4.0;
            p.x += dt * (p.vx + 0.04 * wind + 0.02 * p.spin.sin());
            p.y += dt * p.vy;
        }
        self.petals.retain(|p| p.y < 1.0 && p.x > -0.05 && p.x < 1.05);

        // Luciérnagas de noche, entre los troncos.
        let night = 1.0 - self.scenery.atmosphere().daylight;
        let want = (night * MAX_FLIES as f32) as usize;
        while self.flies.len() < want {
            self.flies.push(Firefly { x: self.rng.next(), y: self.rng.range(0.55, 0.92), phase: self.rng.range(0.0, TAU) });
        }
        self.flies.truncate(want.max(self.flies.len().saturating_sub(1)));
        for (i, fly) in self.flies.iter_mut().enumerate() {
            fly.phase += dt * (0.8 + 0.1 * (i % 5) as f32);
            fly.x = (fly.x + dt * 0.01 * (fly.phase * 0.7).sin()).rem_euclid(1.0);
            fly.y += dt * 0.008 * (fly.phase * 1.3).cos();
        }

        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    fn leaf_color(&self, layer: usize, pal: &Palette) -> Rgb {
        let day = self.scenery.atmosphere().daylight;
        let base = [Rgb(0.13, 0.30, 0.30), Rgb(0.13, 0.42, 0.22), Rgb(0.20, 0.55, 0.18)][layer].mix(pal.leaf, 0.2);
        // El fondo se pierde en la bruma del cielo.
        let haze = [0.45, 0.18, 0.0][layer];
        base.mix(self.scenery.sky(0.9), haze).scale(0.30 + 0.70 * day.max(0.15))
    }

    fn draw_tree(&self, canvas: &mut Canvas, t: &Tree, pal: &Palette) {
        let (g, bloom) = self.growth(t);
        if g <= 0.01 {
            return;
        }
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let day = self.scenery.atmosphere().daylight;
        let size = t.height * h * (0.15 + 0.85 * g);
        let (bx, by) = (t.x * w, GROUND[t.layer] * h);
        let haze = [0.45, 0.18, 0.0][t.layer];
        let bark = Rgb(0.28, 0.18, 0.12).mix(self.scenery.sky(0.9), haze).scale(0.35 + 0.65 * day.max(0.2));
        let sway = (self.sway + t.x * 7.0).sin() * (0.4 + self.bands.bass) * 0.025 * size;
        // Tronco: se angosta hacia arriba y se inclina un poco.
        let trunk_top = 0.62 * size;
        let base_w = (size * 0.06).max(1.0);
        for k in 0..trunk_top as i32 {
            let v = k as f32 / trunk_top.max(1.0);
            let half = (base_w * (1.0 - 0.6 * v) / 2.0).max(0.5);
            let x = bx + t.lean * size * v + sway * v * v;
            for dx in -(half.ceil() as i32)..=half.floor() as i32 {
                canvas.solid(x as i32 + dx, by as i32 - k, bark.scale(if dx < 0 { 0.8 } else { 1.0 }));
            }
        }
        // Copas: círculos con luz desde arriba a la izquierda y textura de hojas.
        let leaf = self.leaf_color(t.layer, pal);
        let flower = flower_color(t.species, pal).mix(self.scenery.sky(0.9), haze).scale(0.40 + 0.60 * day.max(0.25));
        for (i, &(dx, dy, r)) in t.blobs.iter().enumerate() {
            let (cx, cy) = (bx + dx * size + t.lean * size * dy + sway * dy * 1.3, by - dy * size);
            let rad = r * size;
            if rad < 0.6 {
                continue;
            }
            for py in (cy - rad) as i32..=(cy + rad) as i32 {
                for px in (cx - rad) as i32..=(cx + rad) as i32 {
                    let (ux, uy) = ((px as f32 + 0.5 - cx) / rad, (py as f32 + 0.5 - cy) / rad);
                    let d = ux * ux + uy * uy;
                    // Borde irregular, como hojas sueltas.
                    let n = hash(px as u32 ^ self.seed ^ (i as u32 * 7919), py as u32);
                    if d > 1.0 - 0.25 * n {
                        continue;
                    }
                    let light = (0.75 - 0.35 * (ux + uy)).clamp(0.4, 1.2);
                    let mut c = leaf.scale(light * (0.88 + 0.24 * n));
                    // Flores sobre la copa: más mientras más florece.
                    if bloom > 0.0 && n < 0.45 * bloom && d < 0.92 {
                        c = flower.scale(0.85 + 0.3 * light);
                    }
                    canvas.solid(px, py, c);
                }
            }
        }
    }

    fn draw_ground(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let day = self.scenery.atmosphere().daylight;
        // Lomas del fondo con bruma, el suelo medio y el del frente, cada uno más verde.
        for (layer, &ground) in GROUND.iter().enumerate() {
            let c = self.leaf_color(layer, pal).scale(0.75);
            // Cada capa llega hasta abajo: la siguiente la tapa, sin dejar rendijas.
            for x in 0..canvas.w as i32 {
                let u = x as f32 / w;
                let bump = 0.015 * ((u * 9.0 + layer as f32 * 2.0 + self.seed as f32 * 0.001).sin());
                for y in (h * (ground + bump)) as i32..h as i32 {
                    canvas.solid(x, y, c.scale(0.9 + 0.12 * hash(x as u32 ^ self.seed, y as u32)));
                }
            }
        }
        // Flores del suelo: van apareciendo con la canción.
        let colors = [pal.petal, pal.accent, Rgb(1.0, 0.95, 0.85), Rgb(1.0, 0.82, 0.30), Rgb(0.75, 0.55, 1.0)];
        let count = (w * h * (1.0 - GROUND[1]) * 0.08) as u32;
        for i in 0..count {
            if hash(i, self.seed) > smoothstep(0.05, 0.6, self.progress) {
                continue;
            }
            let x = hash(i, 1) * w;
            let y = h * (GROUND[1] + 0.02 + hash(i, 2) * (0.98 - GROUND[1]));
            let c = colors[(hash(i, 3) * 5.0) as usize % 5].scale(0.40 + 0.60 * day.max(0.25));
            let sway = (self.sway + x * 0.3).sin() * self.bands.bass;
            canvas.dot_over(((x + sway) * 2.0) as i32, (y * 2.0) as i32, c, 1.0);
        }
    }

    /// Rayos de sol oblicuos entre los árboles, de día.
    fn draw_rays(&self, canvas: &mut Canvas) {
        let atm = self.scenery.atmosphere();
        let k = atm.daylight * atm.sun * (1.0 - atm.rain) * 0.10;
        if k < 0.005 {
            return;
        }
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        for y in 0..(h * GROUND[2]) as i32 {
            for x in 0..canvas.w as i32 {
                // Bandas diagonales que se mueven muy despacio.
                let u = (x as f32 + y as f32 * 0.6) / w * 6.0 + self.time * 0.05;
                let band = (u.sin() * 0.5 + 0.5).powi(8);
                if band > 0.05 {
                    canvas.paint(x, y, Rgb(1.0, 0.95, 0.75), k * band * (1.0 - y as f32 / h));
                }
            }
        }
    }
}

impl Scene for Forest {
    fn update(&mut self, frame: &Frame, _w: usize, _h: usize) {
        self.advance(frame);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let horizon = (canvas.px_h() as f32 * GROUND[0]).floor().max(1.0);
        let view = View { cx: canvas.w as f32 / 2.0, ground: horizon, scale: horizon.max(4.0), weight: 1.0 };
        let scenery = &self.scenery;
        canvas.clear(|_, y| scenery.sky((y as f32 / horizon).min(1.0)).mix(pal.bg_bottom, 0.08));
        self.scenery.draw_back(canvas, &view, pal);
        self.draw_ground(canvas, pal);
        for layer in 0..3 {
            for t in self.trees.iter().filter(|t| t.layer == layer) {
                self.draw_tree(canvas, t, pal);
            }
        }
        self.draw_rays(canvas);
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let day = self.scenery.atmosphere().daylight;
        for p in &self.petals {
            let c = flower_color(p.color, pal).scale(0.45 + 0.55 * day.max(0.3));
            let (x, y) = ((p.x * w * 2.0) as i32, (p.y * h * 2.0) as i32);
            canvas.dot_over(x, y, c, 1.0);
            if p.spin.sin() > 0.0 {
                canvas.dot_over(x + 1, y, c, 0.8);
            }
        }
        for fly in &self.flies {
            let on = (fly.phase.sin() * 0.5 + 0.5).powi(3);
            let (x, y) = ((fly.x * w) as i32, (fly.y * h) as i32);
            canvas.glow(x, y, Rgb(0.85, 1.0, 0.45), 0.6 * on);
        }
        self.scenery.draw_weather(canvas, &view);
    }

    fn new_track(&mut self) {
        // La transición entre canciones tapa el cambio: el bosque nuevo empieza a brotar.
        self.plant();
        self.petals.clear();
        self.burst = 0.0;
        self.scenery.fade();
    }

    /// Un click hace florecer todo un momento y suelta pétalos desde ahí.
    fn poke(&mut self, x: f32, y: f32) {
        self.burst = self.burst.max(0.8);
        for _ in 0..30.min(MAX_PETALS.saturating_sub(self.petals.len())) {
            let a = self.rng.range(0.0, TAU);
            let s = self.rng.range(0.02, 0.10);
            let color = 1 + (self.rng.next() * 5.0) as u8 % 5;
            self.petals.push(Petal { x, y, vx: a.cos() * s, vy: a.sin().abs() * s * 0.5 + 0.02, color, spin: a });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(f: &mut Forest, bands: &Bands, progress: f32, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            f.update(&Frame { dt: 1.0 / 60.0, bands, progress: Some(progress), song_len: 120.0, playing: true }, 120, 40);
        }
    }

    #[test]
    fn grows_and_blooms_with_the_song() {
        let mut f = Forest::new(1);
        run(&mut f, &Bands::default(), 0.0, 0.1);
        // El fondo ya está; el frente todavía no brota.
        assert!(f.trees.iter().filter(|t| t.layer == 0).all(|t| f.growth(t).0 > 0.99));
        assert!(f.trees.iter().filter(|t| t.layer == 2).all(|t| f.growth(t).0 < 0.05));
        run(&mut f, &Bands::default(), 0.98, 0.1);
        assert!(f.trees.iter().all(|t| f.growth(t).0 > 0.99));
        let flowering = f.trees.iter().filter(|t| t.species > 0).count();
        assert!(flowering > 5, "{flowering}");
        assert!(f.trees.iter().filter(|t| t.species > 0).all(|t| f.growth(t).1 > 0.99));
    }

    #[test]
    fn beats_drop_petals_and_a_climax_blooms_everything() {
        let mut f = Forest::new(2);
        run(&mut f, &Bands { beat: true, intensity: 1.0, ..Bands::default() }, 0.9, 0.5);
        assert!(!f.petals.is_empty());
        let mut young = Forest::new(3);
        run(&mut young, &Bands::default(), 0.35, 0.1);
        run(&mut young, &Bands { climax: true, ..Bands::default() }, 0.35, 1.0 / 60.0);
        assert!(young.trees.iter().filter(|t| t.species > 0 && young.growth(t).0 > 0.0).all(|t| young.growth(t).1 > 0.9));
    }

    #[test]
    fn every_song_plants_a_new_forest() {
        let mut f = Forest::new(4);
        let before: Vec<f32> = f.trees.iter().map(|t| t.x).collect();
        f.new_track();
        let after: Vec<f32> = f.trees.iter().map(|t| t.x).collect();
        assert_ne!(before, after);
    }

    #[test]
    fn draws_at_any_size() {
        let mut f = Forest::new(5);
        let bands = Bands { level: 1.0, bass: 1.0, beat: true, climax: true, intensity: 1.0, ..Bands::default() };
        run(&mut f, &bands, 0.7, 1.0);
        f.poke(0.5, 0.4);
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (220, 60)] {
            let mut canvas = Canvas::new(w, h);
            f.draw(&mut canvas, &Palette::default());
        }
    }
}

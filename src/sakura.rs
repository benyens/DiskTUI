//! Escena "cerezos": un cerezo grande en una loma, con pétalos que caen al ritmo.
//!
//! Las flores se abren durante la primera parte de la canción y se van quedando sin
//! pétalos al final. Cada pulso suelta pétalos, el bombo sacude las ramas y los clímax
//! levantan una ráfaga que llena el aire. El cielo y el clima son los del jardín
//! ([`Scenery`]): de día el sol, de noche la luna y faroles encendidos.

use std::f32::consts::TAU;

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::render::View;
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// Altura de la loma bajo el árbol, como fracción del alto.
const GROUND: f32 = 0.82;
const MAX_PETALS: usize = 900;
/// Las flores terminan de abrirse en este punto de la canción.
const BLOOMED_AT: f32 = 0.40;

/// Racimo de flores de la copa, relativo a la base del tronco y en unidades del alto.
struct Cluster {
    x: f32,
    y: f32,
    r: f32,
    /// Momento (0..1 del florecer) en que se abre.
    opens: f32,
    shade: f32,
}

struct Petal {
    /// Posición en fracciones del ancho y del alto.
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    spin: f32,
    age: f32,
    tone: f32,
}

pub struct Sakura {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    progress: f32,
    bands: Bands,
    clusters: Vec<Cluster>,
    /// Ramas: segmentos (x0, y0, x1, y1, grosor) relativos a la base del tronco.
    branches: Vec<(f32, f32, f32, f32, f32)>,
    petals: Vec<Petal>,
    sway: f32,
    gust: f32,
    beat_pulse: f32,
    /// Canción nueva: los pétalos caen todos y el árbol vuelve a brotar.
    shedding: Option<f32>,
}

impl Sakura {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0x5A4C_0001);
        scenery.bare = true;
        let mut tree = Sakura {
            scenery,
            rng: Rng::new(seed ^ 0x00C4_E221),
            time: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            clusters: Vec::new(),
            branches: Vec::new(),
            petals: Vec::new(),
            sway: 0.0,
            gust: 0.0,
            beat_pulse: 0.0,
            shedding: None,
        };
        tree.grow();
        tree
    }

    /// Arma las ramas y los racimos. Todo en unidades del alto, desde la base del tronco.
    fn grow(&mut self) {
        let rng = &mut self.rng;
        let mut branches = vec![(0.0, 0.0, -0.02, -0.30, 0.035)];
        let mut tips = Vec::new();
        // Tres ramas principales que se abren y se vuelven a dividir.
        for (angle, length) in [(-2.35f32, 0.24f32), (-1.70, 0.22), (-0.80, 0.25)] {
            let (x0, y0) = (-0.02, -0.28);
            let a = angle + rng.range(-0.12, 0.12);
            let (x1, y1) = (x0 + a.cos() * length * 1.2, y0 + a.sin() * length);
            branches.push((x0, y0, x1, y1, 0.020));
            for side in [-0.45f32, 0.40] {
                let b = a + side + rng.range(-0.1, 0.1);
                let l = length * rng.range(0.45, 0.65);
                let (x2, y2) = (x1 + b.cos() * l * 1.2, y1 + b.sin() * l);
                branches.push((x1, y1, x2, y2, 0.011));
                tips.push((x2, y2));
            }
            tips.push((x1, y1));
        }
        // Racimos alrededor de las puntas y a lo largo de las ramas.
        let mut clusters = Vec::new();
        for &(tx, ty) in &tips {
            for _ in 0..9 {
                clusters.push(Cluster {
                    x: tx + rng.range(-0.11, 0.11),
                    y: ty + rng.range(-0.07, 0.05),
                    r: rng.range(0.045, 0.085),
                    opens: rng.range(0.0, 0.9),
                    shade: rng.next(),
                });
            }
        }
        self.branches = branches;
        self.clusters = clusters;
    }

    fn presence(&self) -> f32 {
        self.shedding.map(|t| 1.0 - smoothstep(0.3, 1.0, t)).unwrap_or(1.0)
    }

    /// Qué tan florecido está el árbol: crece al principio y se deshoja al final.
    fn bloom(&self) -> f32 {
        let open = smoothstep(0.0, BLOOMED_AT, self.progress);
        let fall = 1.0 - 0.55 * smoothstep(0.80, 1.0, self.progress);
        open * fall * self.presence()
    }

    /// Dónde está la base del tronco, en fracciones del lienzo.
    fn trunk_base(&self) -> (f32, f32) {
        (0.68, GROUND + 0.02)
    }

    fn release(&mut self, count: usize, from: Option<(f32, f32)>) {
        let (bx, by) = self.trunk_base();
        let room = MAX_PETALS.saturating_sub(self.petals.len());
        for _ in 0..count.min(room) {
            let (x, y) = match from {
                Some(p) => (p.0 + self.rng.range(-0.02, 0.02), p.1 + self.rng.range(-0.02, 0.02)),
                None => {
                    // Desde un racimo abierto al azar.
                    let c = &self.clusters[(self.rng.next() * self.clusters.len() as f32) as usize % self.clusters.len()];
                    // El árbol se dibuja en unidades del alto: en el ancho pesa la mitad.
                    (bx + c.x * 0.5 + self.rng.range(-0.03, 0.03), by + c.y + self.rng.range(-0.03, 0.03))
                }
            };
            let burst = from.is_some();
            self.petals.push(Petal {
                x,
                y,
                vx: if burst { self.rng.range(-0.25, 0.25) } else { self.rng.range(-0.04, 0.02) },
                vy: if burst { self.rng.range(-0.25, 0.05) } else { self.rng.range(0.0, 0.03) },
                spin: self.rng.range(0.0, TAU),
                age: 0.0,
                tone: self.rng.next(),
            });
        }
    }

    fn advance(&mut self, f: &Frame) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        self.beat_pulse = (self.beat_pulse * (-dt / 0.3).exp()).max(if b.beat { 1.0 } else { 0.0 });
        self.gust = (self.gust * (-dt / 1.8).exp()).max(if b.climax { 1.0 } else { 0.0 });
        let wind = self.scenery.atmosphere().wind;
        // Las ramas se mecen con los graves y el viento; el bombo las sacude.
        let target = 0.5 * b.bass + 0.8 * wind + if b.kick { 0.6 } else { 0.0 } + 1.2 * self.gust;
        self.sway += (target - self.sway) * (1.0 - (-dt / 0.35).exp());

        if let Some(t) = &mut self.shedding {
            *t += dt / 1.6;
            if *t >= 1.0 {
                self.shedding = None;
                self.progress = 0.0;
                self.grow();
            }
        } else if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }

        // Pétalos: un goteo según cuánto florece, un puñado en cada pulso, nubes en los
        // clímax. Al final de la canción caen más (el árbol se deshoja).
        let bloom = smoothstep(0.0, BLOOMED_AT, self.progress) * self.presence();
        if f.playing && bloom > 0.05 {
            let late = 1.0 + 2.5 * smoothstep(0.80, 1.0, self.progress);
            let trickle = bloom * late * (1.5 + 5.0 * b.level) * dt;
            let mut count = trickle as usize + usize::from(self.rng.next() < trickle.fract());
            if b.beat {
                count += (2.0 + 4.0 * b.intensity) as usize;
            }
            if b.climax {
                count += 120;
            }
            self.release(count, None);
        }
        if self.shedding.is_some() {
            self.release(6, None);
        }

        let gust = self.gust;
        let time = self.time;
        for p in &mut self.petals {
            p.age += dt;
            p.spin += dt * (2.0 + 3.0 * gust);
            // Caen meciéndose, empujados por el viento y las ráfagas.
            let flutter = 0.05 * (p.spin * 1.3).sin();
            p.vx += dt * (-0.10 * wind - 0.30 * gust * (time * 0.7 + p.tone * 6.0).sin().abs() - 0.03);
            p.vy += dt * (0.06 - 0.12 * gust * (p.tone - 0.5));
            p.vx *= 1.0 - 0.8 * dt;
            p.vy = p.vy.clamp(-0.3, 0.09);
            p.x += (p.vx + flutter) * dt;
            p.y += p.vy * dt;
        }
        self.petals.retain(|p| p.y < 1.02 && p.x > -0.05 && p.age < 30.0);

        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    fn petal_color(pal: &Palette, tone: f32) -> Rgb {
        // Rosa de cerezo, tirado un poco hacia los colores de la carátula.
        let pink = Rgb(1.0, 0.72, 0.82).mix(Rgb(1.0, 0.86, 0.92), tone);
        pink.mix(pal.petal_light, 0.25)
    }

    fn draw_land(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let atm = self.scenery.atmosphere();
        let day = atm.daylight;
        // Montaña nevada lejos, a la izquierda.
        let far = self.scenery.sky(1.0).mix(Rgb(0.30, 0.34, 0.48), 0.5).scale(0.6 + 0.4 * day);
        let snow = Rgb(0.85, 0.88, 0.95).scale(0.45 + 0.55 * day);
        let peak = (w * 0.26, h * 0.40);
        for x in 0..canvas.w {
            let dx = (x as f32 - peak.0).abs() / (w * 0.30);
            // Cono de laderas algo cóncavas, con la cumbre nevada.
            let top = peak.1 + dx.powf(0.85) * h * 0.36;
            let snowline = peak.1 + h * 0.07 + 0.012 * h * (x as f32 * 0.9).sin();
            for y in top as i32..(h * GROUND) as i32 {
                canvas.paint(x as i32, y, if (y as f32) < snowline { snow } else { far }, 1.0);
            }
        }
        // Loma con pasto.
        let grass = Rgb(0.05, 0.10, 0.07).mix(Rgb(0.30, 0.52, 0.28), day).mix(pal.leaf, 0.12);
        for x in 0..canvas.w {
            let u = x as f32 / w;
            let top = h * (GROUND + 0.03 * (u * 5.0).sin() - 0.06 * (1.0 - ((u - 0.68) * 2.2).powi(2)).max(0.0));
            for y in top as i32..h as i32 {
                let depth = (y as f32 - top) / (h - top).max(1.0);
                canvas.paint(x as i32, y, grass.scale(1.0 - 0.3 * depth), 1.0);
            }
        }
        // Faroles de piedra: se encienden de noche y laten con el pulso.
        for (i, u) in [0.18f32, 0.44].into_iter().enumerate() {
            let x = u * w;
            let base = h * (GROUND + 0.03 * (u * 5.0).sin()) - 0.5;
            let stone = Rgb(0.30, 0.30, 0.32).scale(0.4 + 0.6 * day);
            let size = (h * 0.035).clamp(1.0, 4.0);
            for j in 0..(size * 3.0) as i32 {
                let half = if j < size as i32 { size } else { size * 0.5 };
                for k in -(half as i32)..=half as i32 {
                    canvas.solid(x as i32 + k, base as i32 - j, stone);
                }
            }
            let lit = (1.0 - day) * (0.5 + 0.5 * self.beat_pulse) * (0.8 + 0.2 * (self.time * 3.0 + i as f32).sin());
            let fire = Rgb(1.0, 0.75, 0.40);
            let (lx, ly) = (x as i32, (base - size * 2.0) as i32);
            canvas.solid(lx, ly, stone.mix(fire, lit));
            for dy in -5i32..=5 {
                for dx in -8i32..=8 {
                    let d2 = (dx * dx) as f32 / 3.0 + (dy * dy) as f32;
                    canvas.glow(lx + dx, ly + dy, fire, 0.12 * lit * (-d2 / 8.0).exp());
                }
            }
        }
    }

    fn draw_tree(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let atm = self.scenery.atmosphere();
        let (bx, by) = self.trunk_base();
        let (base_x, base_y) = (bx * w, by * h);
        // Las ramas se desplazan más arriba, como si el viento las empujara.
        let bend = |y: f32| -> f32 { self.sway * 1.2 * (-y).max(0.0).powf(1.5) * h * 0.05 * (self.time * 1.7).sin() };
        let to_px = |x: f32, y: f32| -> (f32, f32) { (base_x + x * h * 1.0 + bend(y), base_y + y * h) };
        let bark = Rgb(0.10, 0.06, 0.06).mix(Rgb(0.30, 0.20, 0.17), atm.daylight);
        for &(x0, y0, x1, y1, thick) in &self.branches {
            let (a, b) = (to_px(x0, y0), to_px(x1, y1));
            let steps = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) * 2.0) as i32 + 1;
            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                let r = (thick * h * (1.0 - 0.4 * t)).max(0.6);
                for dx in -(r as i32)..=(r as i32) {
                    canvas.solid((x + dx as f32) as i32, y as i32, bark);
                }
            }
        }
        // Racimos de flores: discos rosados que se abren según el florecer.
        let bloom = self.bloom();
        let light = 0.55 + 0.45 * atm.daylight;
        for c in &self.clusters {
            let open = smoothstep(c.opens, c.opens + 0.15, bloom / 0.9);
            if open < 0.02 {
                continue;
            }
            let (cx, cy) = to_px(c.x, c.y);
            let r = c.r * h * open;
            let tone = Self::petal_color(pal, c.shade).scale(light * (0.85 + 0.15 * self.beat_pulse));
            for dy in -(r as i32)..=(r as i32) {
                for dx in -(r as i32 * 2)..=(r as i32 * 2) {
                    let d = ((dx as f32 / 2.0).powi(2) + (dy as f32).powi(2)).sqrt() / r.max(0.5);
                    if d <= 1.0 {
                        let shade = 0.80 + 0.20 * (1.0 - d) - 0.12 * (dy as f32 / r.max(1.0)).max(0.0);
                        canvas.blend(cx as i32 + dx, cy as i32 + dy, tone.scale(shade), 0.92);
                    }
                }
            }
        }
    }

    fn draw_petals(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let light = 0.6 + 0.4 * self.scenery.atmosphere().daylight;
        for p in &self.petals {
            // Al girar, el pétalo se ve de canto (más oscuro) o de frente.
            let face = 0.6 + 0.4 * p.spin.sin().abs();
            let color = Self::petal_color(pal, p.tone).scale(face * light).add(Rgb::WHITE, 0.15 * self.bands.treble);
            canvas.dot_over((p.x * w * 2.0) as i32, (p.y * h * 2.0) as i32, color, 0.95);
        }
    }
}

impl Scene for Sakura {
    fn update(&mut self, frame: &Frame, _w: usize, _h: usize) {
        self.advance(frame);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let ground = (canvas.px_h() as f32 * GROUND).floor().max(1.0);
        let view = View { cx: canvas.w as f32 / 2.0, ground, scale: ground.max(4.0), weight: 1.0 };
        let scenery = &self.scenery;
        canvas.clear(|_, y| scenery.sky((y as f32 / ground).min(1.0)).mix(pal.bg_bottom, 0.10));
        self.scenery.draw_back(canvas, &view, pal);
        self.draw_land(canvas, pal);
        self.draw_tree(canvas, pal);
        self.draw_petals(canvas, pal);
        self.scenery.draw_weather(canvas, &view);
    }

    fn new_track(&mut self) {
        if self.shedding.is_none() {
            self.shedding = Some(0.0);
        }
        self.scenery.fade();
    }

    fn poke(&mut self, x: f32, y: f32) {
        self.release(40, Some((x, y)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(bands: &Bands, progress: f32) -> Frame<'_> {
        Frame { dt: 1.0 / 60.0, bands, progress: Some(progress), song_len: 120.0, playing: true }
    }

    fn run(tree: &mut Sakura, bands: &Bands, progress: f32, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            tree.update(&frame(bands, progress), 120, 40);
        }
    }

    #[test]
    fn blossoms_open_then_fall_and_beats_release_petals() {
        let mut tree = Sakura::new(1);
        let quiet = Bands::default();
        run(&mut tree, &quiet, 0.0, 1.0);
        assert!(tree.bloom() < 0.05 && tree.petals.is_empty(), "florece antes de tiempo");
        run(&mut tree, &quiet, 0.5, 1.0);
        assert!(tree.bloom() > 0.95);
        let calm = tree.petals.len();
        run(&mut tree, &Bands { beat: true, intensity: 1.0, ..Bands::default() }, 0.5, 1.0);
        assert!(tree.petals.len() > calm + 50, "el pulso no suelta pétalos: {calm} → {}", tree.petals.len());
        run(&mut tree, &quiet, 1.0, 1.0);
        assert!(tree.bloom() < 0.5, "no se deshojó: {}", tree.bloom());
    }

    #[test]
    fn climax_gust_fills_the_air_but_stays_bounded() {
        let mut tree = Sakura::new(2);
        run(&mut tree, &Bands::default(), 0.5, 0.5);
        let before = tree.petals.len();
        tree.update(&frame(&Bands { climax: true, ..Bands::default() }, 0.5), 120, 40);
        assert!(tree.petals.len() >= before + 100);
        run(&mut tree, &Bands { climax: true, beat: true, level: 1.0, ..Bands::default() }, 0.95, 10.0);
        assert!(tree.petals.len() <= MAX_PETALS);
        tree.poke(0.3, 0.3);
    }

    #[test]
    fn draws_at_any_size_and_regrows_on_new_song() {
        let mut tree = Sakura::new(3);
        run(&mut tree, &Bands { beat: true, climax: true, bass: 1.0, ..Bands::default() }, 0.6, 1.0);
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (200, 60)] {
            let mut canvas = Canvas::new(w, h);
            tree.draw(&mut canvas, &Palette::default());
        }
        tree.new_track();
        run(&mut tree, &Bands::default(), 0.6, 1.7);
        assert!(tree.shedding.is_none());
    }
}

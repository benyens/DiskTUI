//! Escena "fogata": un claro del bosque con una fogata que arde al ritmo de la canción.
//!
//! El fuego se enciende al empezar, crece con los graves y la intensidad, y queda en
//! brasas al final. Los agudos sueltan chispas, el pulso hace latir la luz sobre los pinos
//! y los clímax levantan una llamarada. El cielo y el clima son los del jardín
//! ([`Scenery`]).

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::render::View;
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// Alto del suelo, en píxeles.
const GROUND: usize = 5;
/// Cuadrícula de calor de las llamas; la fila 0 es la de arriba.
const FIRE_W: usize = 40;
const FIRE_H: usize = 44;
/// Enfriamiento máximo por fila: fija la altura de la llama para un mismo combustible.
const COOLING: f32 = 0.055;
/// Paso fijo de la simulación del fuego.
const FIRE_STEP: f32 = 1.0 / 30.0;
const MAX_SPARKS: usize = 300;

struct Spark {
    /// Posición relativa a la base del fuego, en fracciones del alto del cielo.
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    life: f32,
    wobble: f32,
}

struct Tree {
    /// Centro, alto y ancho medio, como fracciones del ancho y del alto del cielo.
    x: f32,
    height: f32,
    width: f32,
    tiers: f32,
}

pub struct Campfire {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    progress: f32,
    bands: Bands,
    beat_pulse: f32,
    climax_pulse: f32,
    kick_pulse: f32,
    heat: Vec<f32>,
    step_clock: f32,
    sparks: Vec<Spark>,
    trees: Vec<Tree>,
    /// Canción nueva: el fuego se apaga y se vuelve a encender.
    dying: Option<f32>,
}

impl Campfire {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0xF06A_7A01);
        scenery.bare = true;
        let mut fire = Campfire {
            scenery,
            rng: Rng::new(seed ^ 0x00B0_5C0E),
            time: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            beat_pulse: 0.0,
            climax_pulse: 0.0,
            kick_pulse: 0.0,
            heat: vec![0.0; FIRE_W * FIRE_H],
            step_clock: 0.0,
            sparks: Vec::new(),
            trees: Vec::new(),
            dying: None,
        };
        fire.plant_trees();
        fire
    }

    /// Pinos a los lados; el centro queda libre para el fuego.
    fn plant_trees(&mut self) {
        let rng = &mut self.rng;
        let mut trees: Vec<Tree> = (0..10)
            .map(|i| {
                let side = if i % 2 == 0 { -1.0 } else { 1.0 };
                let away = rng.range(0.22, 0.50);
                Tree {
                    x: 0.5 + side * away,
                    height: rng.range(0.38, 0.62) + 0.45 * (away - 0.22),
                    width: rng.range(0.045, 0.075),
                    tiers: rng.range(3.0, 5.5).floor(),
                }
            })
            .collect();
        // Los más bajos se dibujan primero, como si estuvieran más lejos.
        trees.sort_by(|a, b| a.height.total_cmp(&b.height));
        self.trees = trees;
    }

    fn presence(&self) -> f32 {
        self.dying.map(|t| 1.0 - smoothstep(0.0, 1.0, t)).unwrap_or(1.0)
    }

    /// Cuánto arde el fuego: 0 apagado, ~1 fuego alto, hasta 1.35 en una llamarada.
    fn fuel(&self) -> f32 {
        let b = &self.bands;
        let kindle = smoothstep(0.0, 0.10, self.progress);
        let embers = 1.0 - 0.75 * smoothstep(0.93, 1.0, self.progress);
        let music = 0.36 + 0.45 * b.bass + 0.25 * b.intensity + 0.35 * self.climax_pulse + 0.12 * self.kick_pulse;
        (self.presence() * embers * (0.12 + kindle * music)).min(1.35)
    }

    /// Calor total de las llamas, para las pruebas.
    #[cfg(test)]
    fn blaze(&self) -> f32 {
        self.heat.iter().sum()
    }

    fn step_fire(&mut self) {
        let fuel = self.fuel();
        let lean = (self.scenery.atmosphere().wind * 1.5).clamp(-1.0, 1.0);
        for y in 0..FIRE_H - 1 {
            for x in 0..FIRE_W {
                // Cada celda toma el calor de una vecina de abajo y pierde un poco.
                let drift = (self.rng.next() * 3.0) as i32 - 1 + if self.rng.next() < lean.abs() { -lean.signum() as i32 } else { 0 };
                let from = (x as i32 + drift).clamp(0, FIRE_W as i32 - 1) as usize;
                let below = self.heat[(y + 1) * FIRE_W + from];
                self.heat[y * FIRE_W + x] = (below - self.rng.next() * COOLING).max(0.0);
            }
        }
        // La base: más ancha y caliente cuanto más combustible.
        let reach = 0.28 + 0.34 * fuel.min(1.0);
        for x in 0..FIRE_W {
            let d = ((x as f32 + 0.5) / FIRE_W as f32 - 0.5).abs() * 2.0;
            let heat = if d < reach { fuel * (1.0 - 0.85 * (d / reach).powi(2)) * self.rng.range(0.75, 1.0) } else { 0.0 };
            self.heat[(FIRE_H - 1) * FIRE_W + x] = heat;
        }
    }

    fn spawn_sparks(&mut self, count: usize, fuel: f32) {
        let wind = self.scenery.atmosphere().wind;
        for _ in 0..count.min(MAX_SPARKS.saturating_sub(self.sparks.len())) {
            let spark = Spark {
                x: self.rng.range(-0.06, 0.06),
                y: -self.rng.range(0.04, 0.22) * fuel.max(0.3),
                vx: self.rng.range(-0.07, 0.07) + 0.25 * wind,
                vy: -self.rng.range(0.22, 0.55),
                age: 0.0,
                life: self.rng.range(1.1, 2.6),
                wobble: self.rng.range(0.0, std::f32::consts::TAU),
            };
            self.sparks.push(spark);
        }
    }

    fn advance(&mut self, f: &Frame) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        let decay = |v: f32, tau: f32, hit: bool| (v * (-dt / tau).exp()).max(if hit { 1.0 } else { 0.0 });
        self.beat_pulse = decay(self.beat_pulse, 0.30, b.beat);
        self.climax_pulse = decay(self.climax_pulse, 2.2, b.climax);
        self.kick_pulse = decay(self.kick_pulse, 0.20, b.kick);

        if let Some(t) = &mut self.dying {
            *t += dt / 1.3;
            if *t >= 1.0 {
                self.dying = None;
                self.progress = 0.0;
                self.sparks.clear();
                self.plant_trees();
            }
        } else if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }

        self.step_clock += dt;
        while self.step_clock >= FIRE_STEP {
            self.step_clock -= FIRE_STEP;
            self.step_fire();
        }

        // Chispas: un goteo con los agudos y ráfagas con cada golpe.
        let fuel = self.fuel();
        let trickle = fuel * (3.0 + 28.0 * b.treble) * dt;
        let mut count = trickle as usize + usize::from(self.rng.next() < trickle.fract());
        count += if b.climax { 45 } else { 0 } + if b.section { 12 } else { 0 };
        count += if b.hat { 3 } else { 0 } + if b.kick { 2 } else { 0 };
        if fuel > 0.15 {
            self.spawn_sparks(count, fuel);
        }
        for s in &mut self.sparks {
            s.age += dt;
            s.x += (s.vx + 0.05 * (self.time * 3.0 + s.wobble).sin()) * dt;
            s.y += s.vy * dt;
            s.vy *= 1.0 - 0.35 * dt;
        }
        self.sparks.retain(|s| s.age < s.life);

        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    /// Color de la llama según su calor, teñido con la carátula.
    fn flame(heat: f32, pal: &Palette) -> Rgb {
        let deep = pal.petal.scale(0.35);
        let body = pal.petal.mix(Rgb(1.0, 0.35, 0.08), 0.55);
        let bright = pal.accent.mix(Rgb(1.0, 0.75, 0.20), 0.60);
        let core = Rgb(1.0, 0.96, 0.80);
        if heat < 0.25 {
            deep.mix(body, heat / 0.25)
        } else if heat < 0.55 {
            body.mix(bright, (heat - 0.25) / 0.30)
        } else {
            bright.mix(core, ((heat - 0.55) / 0.45).min(1.0))
        }
    }

    fn draw_tree(&self, canvas: &mut Canvas, tree: &Tree, ground: f32, light: f32, warm: Rgb) {
        let atm = self.scenery.atmosphere();
        let w = canvas.w as f32;
        let (cx, fire_x) = (tree.x * w, w * 0.5);
        let tall = (tree.height * ground).max(4.0);
        let half = (tree.width * w).max(2.0);
        let needles = Rgb(0.03, 0.07, 0.06).mix(Rgb(0.10, 0.30, 0.16), atm.daylight * 0.8).mix(self.scenery.sky(1.0), 0.12);
        let toward = (fire_x - cx).signum();
        let near = (-((cx - fire_x) / (0.42 * w)).powi(2)).exp();
        for j in 0..tall as i32 {
            let v = j as f32 / tall;
            let y = (ground - tall) as i32 + j;
            // Tronco abajo; arriba, pisos de ramas que se abren hacia el suelo.
            let reach = if v > 0.92 { 1.0 } else { half * (0.12 + 0.88 * v) * (0.50 + 0.50 * (v * tree.tiers).fract()) };
            for i in -(reach.round() as i32)..=reach.round() as i32 {
                let facing = if i as f32 * toward > 0.0 { 1.0 } else { 0.25 };
                let lit = light * near * facing * (0.25 + 0.75 * v) * 0.30;
                canvas.blend(cx as i32 + i, y, needles.add(warm, lit), 1.0);
            }
        }
    }

    fn draw_tent(&self, canvas: &mut Canvas, ground: f32, light: f32, warm: Rgb, pal: &Palette) {
        let w = canvas.w as f32;
        if w < 60.0 {
            return;
        }
        let atm = self.scenery.atmosphere();
        let (cx, half, tall) = (w * 0.30, (w * 0.065).max(4.0), (ground * 0.17).max(4.0));
        let cloth = pal.petal.scale(0.22 + 0.50 * atm.daylight);
        for j in 0..tall as i32 {
            let v = (j as f32 + 1.0) / tall;
            let reach = (half * v).round() as i32;
            for i in -reach..=reach {
                // El lado derecho mira al fuego; la puerta es una rendija oscura.
                let lit = if i > 0 { light * 0.40 * (i as f32 / half).clamp(0.3, 1.0) } else { light * 0.08 };
                let door = i.abs() <= (reach / 6) && v > 0.35;
                let color = if door { cloth.scale(0.35) } else { cloth.add(warm, lit) };
                canvas.blend(cx as i32 + i, (ground - tall) as i32 + j, color, 1.0);
            }
        }
    }

    fn draw_fire(&self, canvas: &mut Canvas, ground: f32, pal: &Palette) {
        let w = canvas.w as f32;
        let cx = w * 0.5;
        let (fw, fh) = ((ground * 0.40).max(6.0), (ground * 0.60).max(6.0));
        // Leños cruzados y un anillo de piedras.
        let log = Rgb(0.20, 0.12, 0.07);
        let span = (fw * 0.30).max(3.0) as i32;
        for k in -span..=span {
            let rise = (k.abs() as f32 * 0.22) as i32;
            canvas.blend(cx as i32 + k, ground as i32 - 1 - rise, log, 1.0);
            canvas.blend(cx as i32 + k, ground as i32 - 1 - (span as f32 * 0.22) as i32 + rise, log.scale(0.8), 1.0);
        }
        for k in (-span - 2..=span + 2).step_by(2) {
            canvas.blend(cx as i32 + k, ground as i32, Rgb(0.30, 0.30, 0.32), 1.0);
        }
        for j in 0..fh as i32 {
            for i in 0..fw as i32 {
                let (gx, gy) = ((i as f32 / fw * FIRE_W as f32) as usize, (j as f32 / fh * FIRE_H as f32) as usize);
                let heat = self.heat[gy.min(FIRE_H - 1) * FIRE_W + gx.min(FIRE_W - 1)];
                if heat > 0.04 {
                    let (x, y) = ((cx - fw / 2.0) as i32 + i, (ground - fh) as i32 + j);
                    canvas.blend(x, y, Self::flame(heat, pal), smoothstep(0.04, 0.22, heat));
                }
            }
        }
    }
}

impl Scene for Campfire {
    fn update(&mut self, frame: &Frame, _w: usize, _h: usize) {
        self.advance(frame);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let ph = canvas.px_h();
        let ground = ph.saturating_sub(GROUND).max(1) as f32;
        let w = canvas.w as f32;
        let view = View { cx: w / 2.0, ground, scale: ground.max(4.0), weight: 1.0 };
        let atm = self.scenery.atmosphere();
        let fuel = self.fuel();
        // La luz del fuego: late con el pulso y casi no se nota de día.
        let light = fuel.min(1.2) * (0.75 + 0.25 * self.beat_pulse) * (1.0 - 0.6 * atm.daylight);
        let warm = Self::flame(0.6, pal);
        let scenery = &self.scenery;
        canvas.clear(|_, y| scenery.sky((y as f32 / ground).min(1.0)).mix(pal.bg_bottom, 0.12).add(warm, 0.03 * light));
        self.scenery.draw_back(canvas, &view, pal);

        // Resplandor alrededor del fuego.
        let radius = ground * (0.30 + 0.25 * light);
        let (fx, fy) = (w * 0.5, ground - ground * 0.12);
        for dy in -(radius as i32)..=radius as i32 {
            for dx in -(radius as i32 * 2)..=radius as i32 * 2 {
                let r = ((dx as f32 * 0.5).powi(2) + (dy as f32).powi(2)).sqrt() / radius;
                if r < 1.0 {
                    canvas.glow(fx as i32 + dx, fy as i32 + dy, warm, 0.16 * light * (1.0 - r).powi(2));
                }
            }
        }

        for tree in &self.trees {
            self.draw_tree(canvas, tree, ground, light, warm);
        }
        self.draw_tent(canvas, ground, light, warm, pal);

        // Suelo: pasto oscuro, iluminado cerca del fuego.
        let turf = Rgb(0.04, 0.06, 0.04).mix(Rgb(0.18, 0.32, 0.15), atm.daylight * 0.8);
        for y in ground as usize..ph {
            for x in 0..canvas.w {
                let near = (-((x as f32 - fx) / (0.26 * w)).powi(2)).exp();
                let depth = 1.0 - 0.12 * (y as f32 - ground);
                canvas.blend(x as i32, y as i32, turf.scale(depth.max(0.4)).add(warm, 0.38 * light * near), 1.0);
            }
        }

        self.draw_fire(canvas, ground, pal);
        for s in &self.sparks {
            let fade = 1.0 - s.age / s.life;
            let color = Self::flame(0.25 + 0.6 * fade, pal);
            let (x, y) = (fx + s.x * ground * 2.0, ground + s.y * ground);
            if y > 0.0 {
                canvas.dot_over((x * 2.0) as i32, (y * 2.0) as i32, color, fade * self.presence());
            }
        }
        self.scenery.draw_front(canvas, &view, pal);
    }

    fn new_track(&mut self) {
        if self.dying.is_none() {
            self.dying = Some(0.0);
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

    fn burn(fire: &mut Campfire, bands: &Bands, progress: f32, seconds: f32) -> f32 {
        for _ in 0..(seconds * 60.0) as usize {
            fire.update(&frame(bands, progress), 120, 40);
        }
        fire.blaze()
    }

    #[test]
    fn fire_kindles_grows_with_bass_and_ends_in_embers() {
        let mut fire = Campfire::new(1);
        let quiet = Bands::default();
        let loud = Bands { bass: 0.9, intensity: 0.8, level: 0.8, ..Bands::default() };
        let spark = burn(&mut fire, &quiet, 0.0, 3.0);
        let calm = burn(&mut fire, &quiet, 0.3, 3.0);
        let high = burn(&mut fire, &loud, 0.3, 3.0);
        let embers = burn(&mut fire, &quiet, 1.0, 3.0);
        assert!(spark < calm, "no prendió: {spark} vs {calm}");
        assert!(high > calm * 1.5, "los graves no avivan: {calm} vs {high}");
        assert!(embers < calm * 0.6, "no quedó en brasas: {embers} vs {calm}");
    }

    #[test]
    fn climax_throws_sparks() {
        let mut fire = Campfire::new(2);
        let quiet = Bands::default();
        burn(&mut fire, &quiet, 0.4, 1.0);
        let before = fire.sparks.len();
        let climax = Bands { climax: true, ..Bands::default() };
        fire.update(&frame(&climax, 0.4), 120, 40);
        assert!(fire.sparks.len() >= before + 40, "{} → {}", before, fire.sparks.len());
        burn(&mut fire, &Bands { treble: 1.0, hat: true, ..Bands::default() }, 0.4, 20.0);
        assert!(fire.sparks.len() <= MAX_SPARKS);
    }

    #[test]
    fn draws_at_any_size() {
        let mut fire = Campfire::new(3);
        let bands = Bands { bass: 0.8, treble: 0.7, level: 0.7, intensity: 0.8, climax: true, beat: true, hat: true, ..Bands::default() };
        burn(&mut fire, &bands, 0.5, 1.0);
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (200, 60)] {
            let mut canvas = Canvas::new(w, h);
            fire.draw(&mut canvas, &Palette::default());
        }
    }

    #[test]
    fn new_song_puts_the_fire_out_and_lights_it_again() {
        let mut fire = Campfire::new(4);
        let bands = Bands { bass: 0.6, ..Bands::default() };
        let lit = burn(&mut fire, &bands, 0.5, 3.0);
        fire.new_track();
        burn(&mut fire, &bands, 0.5, 1.2);
        assert!(fire.fuel() < 0.05, "sigue ardiendo: {}", fire.fuel());
        let again = burn(&mut fire, &bands, 0.5, 4.0);
        assert!(fire.dying.is_none());
        assert!(again > lit * 0.6, "no volvió a prender: {lit} vs {again}");
    }
}

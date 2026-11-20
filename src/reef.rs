//! Escena "fondo del mar": medusas que laten con el bombo en un fondo marino.
//!
//! Aparecen medusas a medida que avanza la canción; cada bombo las hace contraerse e
//! impulsarse hacia arriba. Los agudos sueltan burbujas, las algas se mecen con el pulso y
//! los clímax encienden todo con bioluminiscencia y hacen cruzar un cardumen. De día entra
//! luz desde la superficie; de noche, solo brillan los animales. El ánimo de la canción
//! decide cuánta luz hay y qué tan agitada está el agua ([`Scenery`], sin dibujar su cielo).

use std::f32::consts::TAU;

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

const MAX_JELLIES: usize = 6;
const MAX_BUBBLES: usize = 160;
/// Alto del fondo de arena, en píxeles.
const SAND: usize = 4;
const SCHOOL_TIME: f32 = 6.0;

struct Jelly {
    /// Posición como fracción del ancho y del alto; `y` = 0 arriba.
    x: f32,
    y: f32,
    vy: f32,
    size: f32,
    /// Avance de la canción en que aparece.
    birth: f32,
    /// Contracción: 1 recién latió, baja a 0.
    squeeze: f32,
    hue: f32,
    drift: f32,
}

struct Bubble {
    x: f32,
    y: f32,
    speed: f32,
    wobble: f32,
}

pub struct Reef {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    progress: f32,
    bands: Bands,
    jellies: Vec<Jelly>,
    bubbles: Vec<Bubble>,
    glow: f32,
    beat_pulse: f32,
    /// Cardumen cruzando: tiempo transcurrido desde el clímax.
    school: Option<f32>,
    school_dir: f32,
    /// Canción nueva: las medusas bajan y se apagan.
    sinking: Option<f32>,
    weeds: Vec<(f32, f32)>,
}

impl Reef {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0x2EEF_0001);
        scenery.bare = true;
        let mut reef = Reef {
            scenery,
            rng: Rng::new(seed ^ 0x00DE_E9A5),
            time: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            jellies: Vec::new(),
            bubbles: Vec::new(),
            glow: 0.0,
            beat_pulse: 0.0,
            school: None,
            school_dir: 1.0,
            sinking: None,
            weeds: Vec::new(),
        };
        reef.populate();
        reef
    }

    fn populate(&mut self) {
        let rng = &mut self.rng;
        self.jellies = (0..MAX_JELLIES)
            .map(|i| Jelly {
                x: rng.range(0.12, 0.88),
                y: rng.range(0.35, 0.75),
                vy: 0.0,
                size: if i == 0 { 1.25 } else { rng.range(0.6, 1.05) },
                birth: if i == 0 { 0.0 } else { 0.05 + 0.55 * i as f32 / MAX_JELLIES as f32 + rng.range(-0.04, 0.04) },
                squeeze: 0.0,
                hue: rng.next(),
                drift: rng.range(0.0, TAU),
            })
            .collect();
        self.weeds = (0..14).map(|_| (rng.range(0.02, 0.98), rng.range(0.10, 0.30))).collect();
        self.bubbles.clear();
    }

    fn presence(&self) -> f32 {
        self.sinking.map(|t| 1.0 - smoothstep(0.0, 1.0, t)).unwrap_or(1.0)
    }

    /// Medusas que ya aparecieron, para las pruebas.
    #[cfg(test)]
    fn visible(&self) -> usize {
        self.jellies.iter().filter(|j| self.progress >= j.birth).count()
    }

    fn advance(&mut self, f: &Frame) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        self.beat_pulse = (self.beat_pulse * (-dt / 0.3).exp()).max(if b.beat { 1.0 } else { 0.0 });
        self.glow = (self.glow * (-dt / 2.2).exp()).max(if b.climax { 1.0 } else { 0.0 });

        if let Some(t) = &mut self.sinking {
            *t += dt / 1.4;
            if *t >= 1.0 {
                self.sinking = None;
                self.progress = 0.0;
                self.populate();
            }
        } else if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }

        let storm = self.scenery.atmosphere().storm;
        let progress = self.progress;
        let time = self.time;
        let sinking = self.sinking.is_some();
        for (i, j) in self.jellies.iter_mut().enumerate() {
            if progress < j.birth {
                continue;
            }
            // Cada bombo (o un pulso, si no hay bombo) las hace contraerse e impulsarse.
            let hit = b.kick || (b.beat && i % 2 == 1);
            if hit && f.playing && j.squeeze < 0.5 {
                j.squeeze = 1.0;
                j.vy -= 0.07 * (0.6 + 0.6 * b.bass) / j.size.max(0.5);
            }
            j.squeeze *= (-dt / 0.28).exp();
            // Se hunden despacio; al irse la canción, más rápido.
            j.vy += dt * if sinking { 0.25 } else { 0.035 };
            j.vy *= 1.0 - 1.2 * dt;
            j.y += j.vy * dt;
            j.x += dt * (0.010 * (time * 0.3 + j.drift).sin() + 0.02 * storm * (time * 0.7 + j.drift).cos());
            if j.y < 0.24 {
                j.y = 0.24;
                j.vy = j.vy.abs() * 0.3;
            }
            // Sin música no se hunden más allá del fondo.
            j.y = j.y.min(if sinking { 1.2 } else { 0.78 });
            j.x = j.x.rem_euclid(1.0);
        }

        // Burbujas: con los agudos, desde las algas y las medusas.
        let rate = (2.0 + 30.0 * b.treble + if b.hat { 8.0 } else { 0.0 }) * dt * self.presence();
        let mut count = rate as usize + usize::from(self.rng.next() < rate.fract());
        if b.climax {
            count += 25;
        }
        for _ in 0..count.min(MAX_BUBBLES.saturating_sub(self.bubbles.len())) {
            let x = if self.rng.next() < 0.5 { self.weeds[(self.rng.next() * self.weeds.len() as f32) as usize % self.weeds.len()].0 } else { self.rng.next() };
            self.bubbles.push(Bubble { x, y: self.rng.range(0.85, 1.0), speed: self.rng.range(0.08, 0.22), wobble: self.rng.range(0.0, TAU) });
        }
        for bub in &mut self.bubbles {
            bub.y -= bub.speed * dt;
            bub.x += 0.004 * (time * 3.0 + bub.wobble).sin() * dt * 10.0;
        }
        self.bubbles.retain(|bub| bub.y > 0.0);

        if b.climax {
            self.school = Some(0.0);
            self.school_dir = if self.rng.next() < 0.5 { 1.0 } else { -1.0 };
        }
        if let Some(t) = &mut self.school {
            *t += dt;
            if *t > SCHOOL_TIME {
                self.school = None;
            }
        }

        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    fn draw_water(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let atm = self.scenery.atmosphere();
        let day = atm.daylight;
        let top = Rgb(0.02, 0.06, 0.14).mix(Rgb(0.10, 0.45, 0.62), day).mix(pal.bg_top, 0.10);
        let deep = Rgb(0.00, 0.01, 0.04).mix(Rgb(0.02, 0.14, 0.28), day);
        let glow = self.glow;
        let tint = pal.petal.scale(0.25);
        canvas.clear(|_, y| top.mix(deep, (y as f32 / h).powf(0.8)).add(tint, 0.04 * glow));

        // Rayos de luz desde la superficie, que se mecen; con tormenta, titilan.
        let light = day * (1.0 - 0.6 * atm.storm) + 0.15 * atm.moon * (1.0 - day);
        if light > 0.02 {
            let flicker = 1.0 - 0.5 * atm.storm * (0.5 + 0.5 * (self.time * 7.0).sin());
            let ray_color = Rgb(0.85, 0.95, 1.0).mix(pal.petal_light, 0.15);
            for y in 0..h as usize {
                let v = y as f32 / h;
                for x in 0..canvas.w {
                    let u = x as f32 / w;
                    let slant = u + v * 0.25;
                    let rays = (slant * 9.0 + (self.time * 0.25).sin() * 0.6).sin() * (slant * 23.0 - self.time * 0.4).sin();
                    let k = smoothstep(0.35, 1.0, rays) * (1.0 - v).powf(1.6) * 0.18 * light * flicker;
                    if k > 0.003 {
                        canvas.glow(x as i32, y as i32, ray_color, k);
                    }
                }
            }
        }

        // Superficie: una franja ondulada arriba, más movida con viento.
        let ripple = 0.6 + 2.0 * atm.wind + 2.0 * atm.storm;
        for x in 0..canvas.w {
            let y = 1.0 + ripple * 0.5 * (1.0 + (x as f32 * 0.21 + self.time * 1.7).sin());
            for yy in 0..y as i32 {
                canvas.blend(x as i32, yy, Rgb(0.7, 0.85, 0.95), 0.10 + 0.25 * light);
            }
        }

        // Arena y algas.
        let sand = Rgb(0.10, 0.09, 0.08).mix(Rgb(0.58, 0.52, 0.38), day * 0.8);
        for y in (h as usize).saturating_sub(SAND)..h as usize {
            for x in 0..canvas.w {
                let grain = 0.92 + 0.08 * ((x * 7 + y * 13) % 5) as f32 / 4.0;
                canvas.paint(x as i32, y as i32, sand.scale(grain), 1.0);
            }
        }
        let weed = Rgb(0.03, 0.12, 0.08).mix(Rgb(0.16, 0.45, 0.25), day).mix(pal.leaf, 0.3);
        let sway = 0.6 + 0.8 * self.bands.level + 1.2 * atm.wind;
        let floor = h - SAND as f32;
        for (i, (x, tall)) in self.weeds.iter().enumerate() {
            let height = tall * h * (0.6 + 0.4 * smoothstep(0.0, 0.3, self.progress));
            let steps = (height * 2.0) as i32;
            for s in 0..steps {
                let t = s as f32 / steps.max(1) as f32;
                let bend = sway * t * t * ((self.time * 1.3 + i as f32) .sin() * 3.0 + self.beat_pulse * 1.5);
                let px = x * w + bend;
                let py = floor - t * height;
                canvas.dot_over((px * 2.0) as i32, (py * 2.0) as i32, weed.scale(0.7 + 0.5 * t), 1.0);
            }
        }
    }

    fn draw_jelly(&self, canvas: &mut Canvas, j: &Jelly, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let k = self.presence() * smoothstep(j.birth, j.birth + 0.03, self.progress);
        if k < 0.02 {
            return;
        }
        let atm = self.scenery.atmosphere();
        let (cx, cy) = (j.x * w, j.y * h);
        let r = (h * 0.075 * j.size).max(2.0);
        // Al latir, la campana se angosta y se alarga.
        let (rx, ry) = (r * 1.6 * (1.0 - 0.25 * j.squeeze), r * (1.0 + 0.20 * j.squeeze));
        let body = pal.petal.mix(pal.petal_light, j.hue * 0.6).mix(pal.accent, 0.15 * j.hue);
        let lum = 0.35 + 0.65 * (1.0 - atm.daylight) + 0.6 * self.glow + 0.3 * j.squeeze;
        // Halo: bioluminiscencia, fuerte de noche y en los clímax.
        let halo = r * 3.0;
        for dy in -(halo as i32)..=halo as i32 {
            for dx in -(halo as i32 * 2)..=halo as i32 * 2 {
                let d = ((dx as f32 / 2.0).powi(2) + (dy as f32).powi(2)).sqrt() / halo;
                if d < 1.0 {
                    canvas.glow(cx as i32 + dx, cy as i32 + dy, body, k * 0.10 * lum * (1.0 - d).powi(2));
                }
            }
        }
        // Campana: media elipse, más clara arriba y translúcida.
        for dy in -(ry as i32)..=(ry * 0.25) as i32 {
            for dx in -(rx as i32)..=rx as i32 {
                let (u, v) = (dx as f32 / rx, dy as f32 / ry);
                let inside = u * u + v * v;
                if inside <= 1.0 {
                    let shade = 0.55 + 0.45 * (1.0 - inside) + 0.2 * (-v).max(0.0);
                    let color = body.scale(shade * (0.6 + 0.4 * lum.min(1.2)));
                    canvas.blend(cx as i32 + dx, cy as i32 + dy, color, k * (0.55 + 0.25 * (1.0 - inside)));
                }
            }
        }
        // Tentáculos: líneas onduladas que se arrastran detrás del impulso.
        let trail = r * (3.0 + 1.5 * (j.vy.min(0.0) * -10.0).min(1.0));
        for t in 0..5 {
            let base = cx + (t as f32 - 2.0) / 2.0 * rx * 0.7;
            let steps = (trail * 2.0) as i32;
            for s in 0..steps {
                let q = s as f32 / steps.max(1) as f32;
                let wave = (q * 5.0 - self.time * 2.5 + t as f32 + j.drift).sin() * r * 0.35 * q;
                let (x, y) = (base + wave, cy + ry * 0.25 + q * trail);
                canvas.dot_over((x * 2.0) as i32, (y * 2.0) as i32, body.mix(Rgb::WHITE, 0.25), k * (1.0 - q) * 0.9);
            }
        }
    }

    fn draw_school(&self, canvas: &mut Canvas, pal: &Palette) {
        let Some(t) = self.school else { return };
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let across = t / SCHOOL_TIME;
        let silver = pal.accent.mix(Rgb::WHITE, 0.6);
        for i in 0..28 {
            let (a, b) = ((i * 37 % 100) as f32 / 100.0, (i * 61 % 100) as f32 / 100.0);
            let lead = across * 1.4 - 0.2 - a * 0.25;
            let x = if self.school_dir > 0.0 { lead } else { 1.0 - lead } * w;
            let y = h * (0.35 + 0.25 * b + 0.04 * (t * 3.0 + a * 6.0).sin());
            let dir = self.school_dir;
            canvas.dot_over((x * 2.0) as i32, (y * 2.0) as i32, silver, 0.9);
            canvas.dot_over((x * 2.0 - dir) as i32, (y * 2.0) as i32, silver, 0.9);
            canvas.dot_over((x * 2.0 - dir * 2.0) as i32, (y * 2.0) as i32 + if (t * 8.0 + a * 9.0).sin() > 0.0 { 1 } else { -1 }, silver.scale(0.7), 0.8);
        }
    }
}

impl Scene for Reef {
    fn update(&mut self, frame: &Frame, _w: usize, _h: usize) {
        self.advance(frame);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        self.draw_water(canvas, pal);
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let k = self.presence();
        for bub in &self.bubbles {
            let color = Rgb(0.80, 0.92, 1.0);
            canvas.dot_over((bub.x * w * 2.0) as i32, (bub.y * h * 2.0) as i32, color, 0.75 * k);
        }
        for j in &self.jellies {
            self.draw_jelly(canvas, j, pal);
        }
        self.draw_school(canvas, pal);
    }

    /// Un click suelta burbujas desde ahí.
    fn poke(&mut self, x: f32, y: f32) {
        for _ in 0..20.min(MAX_BUBBLES.saturating_sub(self.bubbles.len())) {
            self.bubbles.push(Bubble {
                x: x + self.rng.range(-0.02, 0.02),
                y: y + self.rng.range(-0.02, 0.02),
                speed: self.rng.range(0.10, 0.25),
                wobble: self.rng.range(0.0, TAU),
            });
        }
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
    fn jellies_appear_with_the_song_and_rise_on_each_kick() {
        let mut reef = Reef::new(1);
        let quiet = Bands::default();
        reef.update(&frame(&quiet, 0.0), 120, 40);
        assert_eq!(reef.visible(), 1);
        reef.update(&frame(&quiet, 0.9), 120, 40);
        assert_eq!(reef.visible(), MAX_JELLIES);

        let before = reef.jellies[0].y;
        let kick = Bands { kick: true, bass: 0.9, ..Bands::default() };
        reef.update(&frame(&kick, 0.9), 120, 40);
        for _ in 0..30 {
            reef.update(&frame(&quiet, 0.9), 120, 40);
        }
        assert!(reef.jellies[0].y < before, "no subió con el bombo: {before} → {}", reef.jellies[0].y);
    }

    #[test]
    fn treble_makes_bubbles_and_climax_lights_up() {
        let mut reef = Reef::new(2);
        let bright = Bands { treble: 1.0, hat: true, ..Bands::default() };
        for _ in 0..60 {
            reef.update(&frame(&bright, 0.5), 120, 40);
        }
        let bubbles = reef.bubbles.len();
        assert!(bubbles > 20, "pocas burbujas: {bubbles}");
        reef.update(&frame(&Bands { climax: true, ..Bands::default() }, 0.5), 120, 40);
        assert!(reef.glow > 0.99 && reef.school.is_some());
        for _ in 0..(60.0 * 60.0) as usize {
            reef.update(&frame(&bright, 0.5), 120, 40);
        }
        assert!(reef.bubbles.len() <= MAX_BUBBLES);
    }

    #[test]
    fn draws_at_any_size() {
        let mut reef = Reef::new(3);
        let bands = Bands { bass: 0.8, treble: 0.7, level: 0.7, kick: true, beat: true, climax: true, ..Bands::default() };
        for _ in 0..30 {
            reef.update(&frame(&bands, 0.8), 80, 24);
        }
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (200, 60)] {
            let mut canvas = Canvas::new(w, h);
            reef.draw(&mut canvas, &Palette::default());
        }
    }
}

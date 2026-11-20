//! Escena "mariposas": una pradera de flores silvestres y una migración de mariposas que
//! cruza en diagonal hacia arriba.
//!
//! Las mariposas entran por abajo y por la izquierda y suben hacia la esquina de arriba a la
//! derecha. El volumen decide cuántas vuelan, el tempo qué tan rápido aletean, cada pulso
//! les da un aletazo que las levanta y los clímax hacen despegar una bandada desde las
//! flores. Las cercanas son más grandes y rápidas que las lejanas. Con lluvia vuelan pocas;
//! de noche se vuelven polillas que brillan apenas.

use std::f32::consts::TAU;

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::render::View;
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// Comienzo de la pradera, como fracción del alto.
const GROUND: f32 = 0.70;
const MAX_BUTTERFLIES: usize = 110;
const MAX_POLLEN: usize = 250;
/// Rumbo de la migración: hacia la derecha y hacia arriba.
const HEADING: (f32, f32) = (0.80, -0.60);

struct Butterfly {
    x: f32,
    y: f32,
    /// 0 lejos, 1 cerca: tamaño y velocidad.
    depth: f32,
    flap: f32,
    /// Fase propia del zigzag.
    wobble: f32,
    tint: u8,
    /// Impulso extra de un despegue (clímax o click), que se apaga solo.
    boost: f32,
    /// Desvío del rumbo, para que la bandada no vuele en filas.
    turn: f32,
    /// Posada en una flor: segundos que le quedan ahí.
    rest: Option<f32>,
}

struct Pollen {
    x: f32,
    y: f32,
    speed: f32,
}

pub struct Meadow {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    progress: f32,
    bands: Bands,
    level: f32,
    beat_pulse: f32,
    /// Acumulador de mariposas por nacer.
    pending: f32,
    butterflies: Vec<Butterfly>,
    pollen: Vec<Pollen>,
    /// Fin de canción: dejan de llegar y las que quedan se apuran.
    leaving: Option<f32>,
    /// Varía las flores y las lomas entre canciones.
    seed: u32,
}

impl Meadow {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0x3EAD_0001);
        scenery.bare = true;
        let mut rng = Rng::new(seed ^ 0x0B07_7E21);
        let seed = (rng.next() * 1e6) as u32;
        Meadow {
            scenery,
            rng,
            time: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            level: 0.0,
            beat_pulse: 0.0,
            pending: 0.0,
            butterflies: Vec::new(),
            pollen: Vec::new(),
            leaving: None,
            seed,
        }
    }

    /// Mariposas por segundo que entran a la escena.
    fn rate(&self) -> f32 {
        if self.leaving.is_some() {
            return 0.0;
        }
        let atm = self.scenery.atmosphere();
        let start = smoothstep(0.0, 0.06, self.progress);
        let end = 1.0 - 0.8 * smoothstep(0.93, 1.0, self.progress);
        (0.8 + 3.6 * self.level + 1.6 * self.bands.intensity) * (1.0 - 0.75 * atm.rain) * start * end
    }

    fn spawn(&mut self, x: f32, y: f32, boost: f32) {
        if self.butterflies.len() >= MAX_BUTTERFLIES {
            return;
        }
        let depth = self.rng.next().powf(1.6);
        self.butterflies.push(Butterfly {
            x,
            y,
            depth,
            flap: self.rng.range(0.0, TAU),
            wobble: self.rng.range(0.0, TAU),
            tint: (self.rng.next() * 5.0) as u8 % 5,
            boost,
            turn: self.rng.range(-0.35, 0.35),
            rest: None,
        });
    }

    /// Entra por el borde de abajo o por el izquierdo, para cruzar en diagonal.
    fn spawn_at_edge(&mut self) {
        let (x, y) = if self.rng.next() < 0.55 {
            (self.rng.range(-0.05, 0.75), 1.03)
        } else {
            (-0.03, self.rng.range(0.25, 1.0))
        };
        self.spawn(x, y, 0.0);
    }

    /// Una bandada que despega desde las flores.
    fn take_off(&mut self, count: usize, from: Option<(f32, f32)>) {
        for _ in 0..count {
            let (x, y) = match from {
                Some((x, y)) => (x + self.rng.range(-0.03, 0.03), y + self.rng.range(-0.03, 0.03)),
                None => (self.rng.range(0.0, 0.85), self.rng.range(GROUND + 0.04, 0.98)),
            };
            let boost = self.rng.range(0.6, 1.0);
            self.spawn(x, y, boost);
        }
    }

    fn advance(&mut self, f: &Frame) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        self.level += (b.level - self.level) * (1.0 - (-dt / 0.5).exp());
        self.beat_pulse = (self.beat_pulse * (-dt / 0.25).exp()).max(if b.beat { 1.0 } else { 0.0 });

        if let Some(t) = &mut self.leaving {
            *t += dt / 2.5;
            if *t >= 1.0 {
                self.leaving = None;
                self.progress = 0.0;
                self.butterflies.clear();
                self.seed = (self.rng.next() * 1e6) as u32;
            }
        } else if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }

        if f.playing {
            self.pending += self.rate() * dt;
            while self.pending >= 1.0 {
                self.pending -= 1.0;
                self.spawn_at_edge();
            }
            if b.climax {
                self.take_off(22, None);
            }
        }

        let tempo = if b.song.bpm > 0.0 { (b.song.bpm / 110.0).clamp(0.6, 1.6) } else { 1.0 };
        let atm = self.scenery.atmosphere();
        let wind = atm.wind;
        let hurry = 1.0 + 2.0 * self.leaving.unwrap_or(0.0);
        let (hx, hy) = HEADING;
        let (time, pulse) = (self.time, self.beat_pulse);
        // En los pasajes tranquilos algunas se posan en las flores; un golpe fuerte o un
        // clímax las hace despegar.
        let calm = self.level < 0.3 && self.leaving.is_none();
        let wake = b.climax || (b.beat && self.level > 0.45) || self.leaving.is_some();
        let rng = &mut self.rng;
        for bf in &mut self.butterflies {
            if let Some(left) = &mut bf.rest {
                *left -= dt;
                bf.flap += dt * 1.5;
                if wake || *left <= 0.0 {
                    bf.rest = None;
                    bf.boost = rng.range(0.5, 0.9);
                }
                continue;
            }
            if calm && bf.boost < 0.1 && bf.y > GROUND + 0.04 && rng.next() < dt * 0.25 {
                bf.rest = Some(rng.range(2.5, 7.0));
                continue;
            }
            // Aletean más rápido con el tempo y el volumen; el pulso da un aletazo.
            bf.flap += dt * (8.0 + 5.0 * self.level + 6.0 * pulse) * tempo * (0.8 + 0.4 * (1.0 - bf.depth));
            let speed = (0.045 + 0.075 * bf.depth) * (0.75 + 0.6 * self.level + 0.8 * bf.boost) * hurry;
            let (c, s) = (bf.turn.cos(), bf.turn.sin());
            let (dx, dy) = (hx * c - hy * s, hx * s + hy * c);
            // Zigzag perpendicular al rumbo y un saltito con cada aletazo.
            let zig = 0.035 * (time * 1.7 + bf.wobble).sin();
            let bob = -0.020 * bf.flap.sin().max(0.0) - 0.03 * pulse * (0.5 + bf.depth);
            bf.x += dt * (dx * speed - dy * zig + 0.03 * wind);
            bf.y += dt * (dy * speed + dx * zig + bob);
            bf.boost *= (-dt / 1.2).exp();
        }
        self.butterflies.retain(|bf| bf.x < 1.08 && bf.y > -0.08 && bf.x > -0.1 && bf.y < 1.1);

        // Polen que sube con los agudos, también en diagonal.
        let rate = (4.0 + 25.0 * b.treble) * dt;
        let count = rate as usize + usize::from(self.rng.next() < rate.fract());
        for _ in 0..count.min(MAX_POLLEN.saturating_sub(self.pollen.len())) {
            self.pollen.push(Pollen { x: self.rng.range(-0.1, 0.9), y: self.rng.range(GROUND, 1.0), speed: self.rng.range(0.015, 0.05) });
        }
        for p in &mut self.pollen {
            p.x += dt * p.speed * hx * 1.2;
            p.y += dt * p.speed * hy;
        }
        self.pollen.retain(|p| p.x < 1.02 && p.y > 0.0);

        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    fn color(&self, tint: u8, pal: &Palette) -> Rgb {
        match tint {
            0 => Rgb(1.0, 0.55, 0.12),            // monarca
            1 => pal.petal.mix(Rgb::WHITE, 0.15),
            2 => pal.accent.mix(Rgb::WHITE, 0.10),
            3 => Rgb(0.45, 0.75, 1.0),            // azul morpho
            _ => Rgb(1.0, 0.95, 0.70),            // limonera
        }
    }

    fn draw_land(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let atm = self.scenery.atmosphere();
        let day = atm.daylight;
        let sd = (self.seed % 1000) as f32 / 160.0;
        // Dos lomas lejanas, azuladas por la distancia.
        for (k, (height, tone)) in [(0.20f32, 0.55f32), (0.10, 0.25)].into_iter().enumerate() {
            let k = k as f32;
            let far = self.scenery.sky(1.0).mix(Rgb(0.20, 0.36, 0.30).mix(pal.leaf, 0.2), 1.0 - tone).scale(0.55 + 0.45 * day);
            for x in 0..canvas.w {
                let u = x as f32 / w;
                // Lomas redondeadas: suma de dos ondas, siempre sobre la pradera.
                let wave = 0.5 + 0.35 * (u * (3.1 + 1.3 * k) + sd + k * 1.7).sin() + 0.15 * (u * (7.3 - k) + sd * 0.7).sin();
                let top = h * (GROUND - height * wave.clamp(0.08, 1.0));
                for y in top as i32..(h * GROUND) as i32 + 1 {
                    canvas.paint(x as i32, y, far, 1.0);
                }
            }
        }
        // La pradera: más clara arriba, con franjas de pasto.
        let grass = Rgb(0.04, 0.09, 0.06).mix(Rgb(0.36, 0.60, 0.26), day).mix(pal.leaf, 0.15);
        let ground = (h * GROUND) as i32;
        for y in ground..h as i32 {
            let depth = (y as f32 - h * GROUND) / (h * (1.0 - GROUND)).max(1.0);
            for x in 0..canvas.w as i32 {
                let stripe = 0.04 * ((x as f32 * 0.21 + y as f32 * 0.9 + (x as f32 * 0.05).sin() * 3.0).sin());
                canvas.paint(x, y, grass.scale(0.85 + 0.25 * depth + stripe), 1.0);
            }
        }
        // Flores silvestres: más chicas y juntas al fondo, se mecen con los graves.
        let flowers = [pal.petal, pal.accent, Rgb(1.0, 0.95, 0.85), Rgb(1.0, 0.85, 0.30), Rgb(0.75, 0.55, 1.0)];
        let sway = 0.6 + 1.6 * self.bands.bass;
        let count = (w * (h * (1.0 - GROUND)) * 0.10) as u32;
        for i in 0..count {
            let hash = (i ^ self.seed).wrapping_mul(2_654_435_761);
            let u = (hash % 1009) as f32 / 1009.0;
            let v = ((hash >> 10) % 1013) as f32 / 1013.0;
            let y = h * GROUND + v.powf(1.4) * h * (1.0 - GROUND);
            let near = (y - h * GROUND) / (h * (1.0 - GROUND)).max(1.0);
            let x = u * w + sway * near * (self.time * 1.4 + u * 9.0).sin();
            let c = flowers[(hash >> 22) as usize % flowers.len()].scale(0.45 + 0.55 * day);
            canvas.dot_over((x * 2.0) as i32, (y * 2.0) as i32, c, 0.55 + 0.45 * near);
            if near > 0.55 {
                canvas.dot_over((x * 2.0) as i32 + 1, (y * 2.0) as i32, c, 0.8);
            }
        }
    }

    /// Una mariposa en (`px`, `py`), en píxeles del lienzo.
    fn draw_butterfly(canvas: &mut Canvas, bf: &Butterfly, px: f32, py: f32, color: Rgb, night: f32) {
        let s = 0.9 + 2.4 * bf.depth;
        // Apertura de las alas: de cerradas (perfil) a extendidas.
        let open = 0.15 + 0.85 * bf.flap.sin().abs();
        let body = Rgb(0.08, 0.06, 0.05);
        let alpha = 0.55 + 0.45 * bf.depth;
        let (rx, ry) = ((2.0 * s + 1.0).ceil() as i32, (s + 1.0).ceil() as i32);
        for dy in -ry..=ry {
            for dx in -rx..=rx {
                let (ax, fy) = (dx.abs() as f32, dy as f32);
                let in_ellipse = |cx: f32, cy: f32, ex: f32, ey: f32| {
                    let (qx, qy) = ((ax - cx) / ex.max(0.45), (fy - cy) / ey.max(0.45));
                    qx * qx + qy * qy <= 1.0
                };
                let upper = in_ellipse(0.35 + 0.9 * s * open, -0.35 * s, 0.95 * s * open, 0.75 * s);
                let lower = in_ellipse(0.25 + 0.6 * s * open, 0.45 * s, 0.6 * s * open, 0.5 * s);
                let (x, y) = (px as i32 + dx, py as i32 + dy);
                if dx == 0 && fy.abs() <= 0.6 * s && s > 1.6 {
                    canvas.paint(x, y, body, alpha);
                } else if upper {
                    canvas.paint(x, y, color, alpha);
                } else if lower {
                    canvas.paint(x, y, color.scale(0.75), alpha);
                }
            }
        }
        // De noche, un halo tenue.
        if night > 0.05 {
            canvas.glow(px as i32, py as i32, color, 0.25 * night * alpha);
        }
    }
}

impl Scene for Meadow {
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
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let day = self.scenery.atmosphere().daylight;
        for p in &self.pollen {
            canvas.dot_over((p.x * w * 2.0) as i32, (p.y * h * 2.0) as i32, pal.accent.mix(Rgb(1.0, 0.95, 0.6), 0.6), 0.5);
        }
        // Las lejanas primero, para que las cercanas pasen por delante.
        let mut order: Vec<&Butterfly> = self.butterflies.iter().collect();
        order.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        for bf in order {
            let color = self.color(bf.tint, pal).scale(0.45 + 0.55 * day);
            Self::draw_butterfly(canvas, bf, bf.x * w, bf.y * h, color, 1.0 - day);
        }
        self.scenery.draw_weather(canvas, &view);
    }

    fn new_track(&mut self) {
        if self.leaving.is_none() {
            self.leaving = Some(0.0);
        }
        self.scenery.fade();
    }

    /// Un click hace despegar una bandada desde ahí.
    fn poke(&mut self, x: f32, y: f32) {
        self.take_off(14, Some((x, y)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(bands: &Bands, progress: f32) -> Frame<'_> {
        Frame { dt: 1.0 / 60.0, bands, progress: Some(progress), song_len: 120.0, playing: true }
    }

    fn run(m: &mut Meadow, bands: &Bands, progress: f32, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            m.update(&frame(bands, progress), 120, 40);
        }
    }

    #[test]
    fn louder_music_brings_more_butterflies() {
        let mut quiet = Meadow::new(1);
        let mut loud = Meadow::new(1);
        run(&mut quiet, &Bands::default(), 0.5, 8.0);
        run(&mut loud, &Bands { level: 1.0, ..Bands::default() }, 0.5, 8.0);
        assert!(loud.butterflies.len() > quiet.butterflies.len() * 2, "{} vs {}", loud.butterflies.len(), quiet.butterflies.len());
        assert!(loud.butterflies.len() <= MAX_BUTTERFLIES);
    }

    #[test]
    fn they_fly_up_and_to_the_right() {
        let mut m = Meadow::new(2);
        m.poke(0.3, 0.8);
        let start: Vec<(f32, f32)> = m.butterflies.iter().map(|b| (b.x, b.y)).collect();
        let bands = Bands::default();
        for _ in 0..90 {
            m.advance(&Frame { dt: 1.0 / 60.0, bands: &bands, progress: Some(0.5), song_len: 120.0, playing: false });
        }
        let (mut dx, mut dy) = (0.0, 0.0);
        for (b, (x, y)) in m.butterflies.iter().zip(&start) {
            dx += b.x - x;
            dy += b.y - y;
        }
        assert!(dx > 0.0 && dy < 0.0, "no van en diagonal hacia arriba: dx {dx}, dy {dy}");
    }

    #[test]
    fn quiet_passages_make_them_land_and_a_climax_wakes_them() {
        let mut m = Meadow::new(5);
        run(&mut m, &Bands { level: 1.0, ..Bands::default() }, 0.5, 6.0);
        run(&mut m, &Bands::default(), 0.5, 6.0);
        let resting = m.butterflies.iter().filter(|b| b.rest.is_some()).count();
        assert!(resting > 0, "ninguna se posó");
        let spot: Vec<(f32, f32)> = m.butterflies.iter().filter(|b| b.rest.is_some()).map(|b| (b.x, b.y)).collect();
        run(&mut m, &Bands::default(), 0.5, 0.5);
        let still = m.butterflies.iter().filter(|b| b.rest.is_some()).filter(|b| spot.contains(&(b.x, b.y))).count();
        assert!(still > 0, "las posadas se movieron");
        run(&mut m, &Bands { climax: true, ..Bands::default() }, 0.5, 1.0 / 60.0);
        assert_eq!(m.butterflies.iter().filter(|b| b.rest.is_some()).count(), 0);
    }

    #[test]
    fn a_climax_makes_a_flock_take_off_and_the_end_clears_them() {
        let mut m = Meadow::new(3);
        run(&mut m, &Bands::default(), 0.5, 1.0);
        let before = m.butterflies.len();
        run(&mut m, &Bands { climax: true, ..Bands::default() }, 0.5, 1.0 / 60.0);
        assert!(m.butterflies.len() >= before + 18);
        m.new_track();
        run(&mut m, &Bands::default(), 0.5, 3.0);
        assert!(m.leaving.is_none());
        assert!(m.butterflies.len() < 10);
    }

    #[test]
    fn draws_at_any_size() {
        let mut m = Meadow::new(4);
        let bands = Bands { level: 1.0, treble: 1.0, bass: 1.0, beat: true, climax: true, ..Bands::default() };
        run(&mut m, &bands, 0.5, 1.0);
        m.poke(0.5, 0.5);
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (200, 60)] {
            let mut canvas = Canvas::new(w, h);
            m.draw(&mut canvas, &Palette::default());
        }
    }
}

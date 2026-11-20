//! Escena "concierto": un escenario al aire libre, de noche.
//!
//! Atrás, una pantalla LED con barras que siguen los graves, medios y agudos. Desde la
//! estructura de arriba bajan haces de luz que barren al ritmo y cambian de color en cada
//! golpe; los parlantes laten con el bombo. Abajo, el público: salta en los golpes fuertes,
//! levanta las manos en el coro y prende las linternas de los celulares en las partes
//! tranquilas. El clímax trae estrobo y confeti.

use std::f32::consts::TAU;

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::render::View;
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// Escenario: bordes, techo, piso. Fracciones del lienzo.
const STAGE_LEFT: f32 = 0.14;
const STAGE_RIGHT: f32 = 0.86;
const TRUSS: f32 = 0.20;
const FLOOR: f32 = 0.62;
/// Desde dónde empieza el público.
const CROWD: f32 = 0.70;
const LIGHTS: usize = 8;
const MAX_CONFETTI: usize = 500;

struct Confetti {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    color: u8,
    spin: f32,
}

pub struct Concert {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    progress: f32,
    bands: Bands,
    /// Barras de la pantalla, suavizadas.
    bars: [f32; 16],
    kick: f32,
    beat: f32,
    climax: f32,
    /// Color de las luces: cambia en cada golpe.
    hue: usize,
    /// Salto del público (0 a 1).
    jump: f32,
    jump_v: f32,
    confetti: Vec<Confetti>,
    /// Cambio de canción: apagón y vuelven las luces.
    blackout: Option<f32>,
    seed: u32,
}

fn hash(a: u32, b: u32) -> f32 {
    let h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    let h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
    (h ^ (h >> 13)) as f32 / u32::MAX as f32
}

impl Concert {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0xC0BC_E001);
        scenery.bare = true;
        let mut rng = Rng::new(seed ^ 0x57A6_E000);
        let seed = (rng.next() * 1e6) as u32;
        Concert {
            scenery,
            rng,
            time: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            bars: [0.0; 16],
            kick: 0.0,
            beat: 0.0,
            climax: 0.0,
            hue: 0,
            jump: 0.0,
            jump_v: 0.0,
            confetti: Vec::new(),
            blackout: None,
            seed,
        }
    }

    /// Colores de las luces: los de la carátula y unos de escenario.
    fn light_color(&self, i: usize, pal: &Palette) -> Rgb {
        let colors = [pal.accent, pal.petal, Rgb(0.35, 0.55, 1.0), Rgb(1.0, 0.25, 0.55), Rgb(0.3, 1.0, 0.75), Rgb(1.0, 0.75, 0.3)];
        colors[(i + self.hue) % colors.len()].mix(Rgb::WHITE, 0.15)
    }

    /// Cuánta luz hay: baja en el apagón entre canciones y al empezar.
    fn power(&self) -> f32 {
        let on = smoothstep(0.0, 0.03, self.progress);
        match self.blackout {
            Some(t) => (1.0 - smoothstep(0.0, 0.3, t)) * on.max(0.0) + smoothstep(0.7, 1.0, t),
            None => on.max(0.15),
        }
    }

    fn advance(&mut self, f: &Frame) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        if let Some(t) = &mut self.blackout {
            *t += dt / 1.6;
            if *t >= 1.0 {
                self.blackout = None;
            }
        }
        if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }
        self.kick = (self.kick * (-dt / 0.15).exp()).max(if b.kick { 1.0 } else { 0.0 });
        self.beat = (self.beat * (-dt / 0.3).exp()).max(if b.beat { 1.0 } else { 0.0 });
        self.climax = (self.climax * (-dt / 2.0).exp()).max(if b.climax { 1.0 } else { 0.0 });
        if b.beat && f.playing {
            self.hue += 1;
        }
        // Barras de la pantalla: cada una mira una mezcla de bandas y se mueve sola.
        for (i, bar) in self.bars.iter_mut().enumerate() {
            let u = i as f32 / 15.0;
            let src = b.bass * (1.0 - u).powi(2) + b.mid * (1.0 - (u - 0.5).abs() * 2.0).max(0.0) + b.treble * u * u;
            let wobble = 0.15 * (self.time * (3.0 + i as f32 * 0.7)).sin().abs();
            let target = (src * 1.4 + wobble * b.level).min(1.0);
            let k = if target > *bar { 0.5 } else { 0.08 };
            *bar += (target - *bar) * k;
        }
        // El público salta con los golpes fuertes, más mientras más intensa la canción.
        if b.kick && f.playing && b.intensity > 0.35 {
            self.jump_v = self.jump_v.max(1.2 + 1.5 * b.intensity);
        }
        self.jump_v -= dt * 9.0;
        self.jump = (self.jump + self.jump_v * dt).max(0.0);
        if self.jump == 0.0 {
            self.jump_v = self.jump_v.max(0.0);
        }
        // Confeti en el clímax.
        if b.climax && f.playing {
            self.throw(150, None);
        }
        for c in &mut self.confetti {
            c.spin += dt * 6.0;
            c.vy += dt * 0.15;
            c.vx *= (-dt * 1.5).exp();
            c.x += dt * (c.vx + 0.02 * c.spin.sin());
            c.y += dt * c.vy.min(0.18);
        }
        self.confetti.retain(|c| c.y < 1.02);
        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    /// Lanza confeti desde los cañones del escenario o desde un punto.
    fn throw(&mut self, count: usize, from: Option<(f32, f32)>) {
        for _ in 0..count.min(MAX_CONFETTI.saturating_sub(self.confetti.len())) {
            let (x, y, vx, vy) = match from {
                Some((x, y)) => (x, y, self.rng.range(-0.25, 0.25), self.rng.range(-0.25, 0.0)),
                None => {
                    let side = if self.rng.next() < 0.5 { STAGE_LEFT } else { STAGE_RIGHT };
                    let dir = if side < 0.5 { 1.0 } else { -1.0 };
                    (side, FLOOR - 0.02, dir * self.rng.range(0.05, 0.45), self.rng.range(-0.55, -0.2))
                }
            };
            self.confetti.push(Confetti { x, y, vx, vy, color: (self.rng.next() * 6.0) as u8, spin: self.rng.range(0.0, TAU) });
        }
    }

    fn draw_stage(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let power = self.power();
        let (l, r) = ((w * STAGE_LEFT) as i32, (w * STAGE_RIGHT) as i32);
        let (top, floor) = ((h * TRUSS) as i32, (h * FLOOR) as i32);
        // Fondo del escenario, apenas iluminado por la pantalla.
        let back = Rgb(0.05, 0.04, 0.07).add(pal.accent, 0.06 * power);
        for y in top..floor {
            for x in l..=r {
                canvas.solid(x, y, back);
            }
        }
        // Pantalla LED: barras con los colores de las luces y una grilla de puntos.
        let (sl, sr) = (w * 0.30, w * 0.70);
        let (st, sb) = (h * 0.27, h * 0.53);
        let strobe = if self.climax > 0.3 && (self.time * 12.0).sin() > 0.6 { self.climax } else { 0.0 };
        for y in st as i32..sb as i32 {
            for x in sl as i32..sr as i32 {
                let u = (x as f32 - sl) / (sr - sl);
                let v = 1.0 - (y as f32 - st) / (sb - st);
                let i = ((u * 16.0) as usize).min(15);
                let lit = v < self.bars[i];
                let grid = if x % 2 == 0 && y % 2 == 0 { 1.0 } else { 0.75 };
                let mut c = if lit { self.light_color(i / 4, pal).scale((0.5 + 0.5 * v) * grid) } else { Rgb(0.04, 0.04, 0.06) };
                c = c.mix(Rgb::WHITE, strobe * 0.8);
                canvas.solid(x, y, c.scale(0.2 + 0.8 * power));
            }
        }
        // Estructura: techo de reticulado y dos torres.
        let truss = Rgb(0.35, 0.35, 0.38).scale(0.4 + 0.4 * power);
        for x in l - 2..=r + 2 {
            for dy in 0..3 {
                let diag = (x + dy) % 4 == 0 || dy != 1;
                if diag {
                    canvas.solid(x, top - 3 + dy, truss);
                }
            }
        }
        for y in top - 3..floor {
            for side in [l - 2, r + 1] {
                canvas.solid(side, y, truss);
                if (y + side) % 3 == 0 {
                    canvas.solid(side + 1, y, truss);
                }
            }
        }
        // Piso del escenario con su borde iluminado.
        let edge = self.light_color(0, pal).scale(0.6 * power + 0.1);
        for x in l - 2..=r + 2 {
            canvas.solid(x, floor, edge);
            for y in floor + 1..floor + 4 {
                canvas.solid(x, y, Rgb(0.06, 0.05, 0.06));
            }
        }
        // Parlantes a los lados: torres de cajas con conos que laten con el bombo.
        let box_w = (w * 0.06).max(3.0) as i32;
        for (k, x0) in [l - 3 - box_w, r + 3].into_iter().enumerate() {
            let rows = ((floor - top) as f32 / (box_w as f32 * 0.9)).max(1.0) as i32;
            for row in 0..rows {
                let y0 = floor - (row + 1) * (box_w * 9 / 10).max(2);
                for y in y0..y0 + (box_w * 9 / 10).max(2) - 1 {
                    for x in x0..x0 + box_w {
                        canvas.solid(x, y, Rgb(0.07, 0.07, 0.08));
                    }
                }
                let (cx, cy) = (x0 as f32 + box_w as f32 / 2.0, y0 as f32 + box_w as f32 * 0.45);
                let cone = box_w as f32 * (0.28 + 0.08 * self.kick);
                for y in (cy - cone) as i32..=(cy + cone) as i32 {
                    for x in (cx - cone) as i32..=(cx + cone) as i32 {
                        let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                        if d <= cone {
                            let ring = if d > cone * 0.7 { 0.22 } else { 0.12 + 0.08 * self.kick };
                            canvas.solid(x, y, Rgb(ring, ring, ring + 0.02).add(self.light_color(k, pal), 0.1 * self.kick * power));
                        }
                    }
                }
            }
        }
        // La banda: siluetas contra la pantalla, que se mecen con el pulso.
        let dark = Rgb(0.02, 0.02, 0.03);
        let bob = (self.beat * 1.5) as i32;
        let unit = ((floor - top) as f32 / 9.0).max(1.0);
        for (x, tall, kind) in [(0.40, 4.0, 0u8), (0.52, 4.6, 1), (0.63, 4.1, 2)] {
            let cx = (w * x) as i32;
            let height = (unit * tall) as i32;
            let base = floor - 1;
            let head = (unit * 0.45).max(1.0) as i32;
            for y in base - height + head * 2..=base {
                let half = if y > base - height / 2 { (unit * 0.35) as i32 } else { (unit * 0.55) as i32 };
                for dx in -half..=half {
                    canvas.solid(cx + dx, y - bob * i32::from(kind != 2), dark);
                }
            }
            for dy in 0..head * 2 {
                for dx in -head..=head {
                    canvas.solid(cx + dx, base - height + dy - bob * i32::from(kind != 2), dark);
                }
            }
            match kind {
                // Micrófono delante de quien canta.
                1 => {
                    for y in base - height + head * 2..=base {
                        canvas.solid(cx + (unit * 0.9) as i32, y, Rgb(0.25, 0.25, 0.27));
                    }
                }
                // Guitarra cruzada.
                0 => {
                    for k in 0..(unit * 2.0) as i32 {
                        canvas.solid(cx - (unit * 0.8) as i32 + k, base - height / 2 - k / 3, Rgb(0.30, 0.12, 0.08));
                    }
                }
                _ => {}
            }
        }
    }

    /// Haces de luz desde el techo: barren al ritmo y cambian de color con cada golpe.
    fn draw_beams(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let power = self.power();
        let energy = 0.35 + 0.65 * self.bands.intensity.max(self.climax);
        let strength = 0.16 * power * energy;
        if strength < 0.005 {
            return;
        }
        let top = h * TRUSS - 2.0;
        let floor = h;
        for i in 0..LIGHTS {
            let u = (i as f32 + 0.5) / LIGHTS as f32;
            let x0 = w * (STAGE_LEFT + (STAGE_RIGHT - STAGE_LEFT) * u);
            let swing = (self.time * (0.6 + 0.25 * (i % 3) as f32) + i as f32 * 1.3).sin();
            let angle = swing * (0.35 + 0.25 * energy) + if i % 2 == 0 { 0.15 } else { -0.15 };
            let color = self.light_color(i, pal);
            canvas.glow(x0 as i32, top as i32, color, 0.6 * power);
            for y in top as i32..floor as i32 {
                let d = y as f32 - top;
                let cx = x0 + d * angle.tan();
                let half = 0.5 + d * 0.06;
                for x in (cx - half) as i32..=(cx + half) as i32 {
                    let k = 1.0 - (x as f32 - cx).abs() / (half + 0.5);
                    let haze = 0.7 + 0.3 * hash(x as u32 ^ self.seed, (y as u32).wrapping_add((self.time * 8.0) as u32));
                    canvas.paint(x, y, color, strength * k * haze * (1.0 - d / (floor - top) * 0.5));
                }
            }
        }
        // Estrobo del clímax: destellos blancos de toda la escena.
        if self.climax > 0.3 && (self.time * 12.0).sin() > 0.85 {
            canvas.wash(Rgb::WHITE, 0.25 * self.climax * power);
        }
    }

    /// El público: cabezas y hombros, manos arriba en el coro, linternas en lo tranquilo.
    fn draw_crowd(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let power = self.power();
        let rows = 3;
        let calm = 1.0 - smoothstep(0.25, 0.5, self.bands.intensity);
        let hands = smoothstep(0.55, 0.85, self.bands.intensity).max(self.climax);
        for row in 0..rows {
            let depth = row as f32 / (rows - 1) as f32;
            let base = h * (CROWD + 0.05 + 0.11 * depth) + 0.10 * h * depth;
            let size = (h * (0.025 + 0.025 * depth)).max(1.5);
            let gap = size * 1.7;
            // Siluetas casi negras; solo el borde de arriba de las cabezas toma la luz.
            let rim = Rgb(0.02, 0.02, 0.03).add(self.light_color(row, pal), 0.35 * power);
            // Las filas de atrás reciben más luz del escenario: así se separan unas de otras.
            let body = Rgb(0.12, 0.11, 0.14).scale(1.0 - 0.7 * depth).add(self.light_color(row + 2, pal), 0.10 * power * (1.0 - depth));
            let n = (w / gap) as i32 + 2;
            for k in 0..n {
                let id = (row as u32 * 1000 + k as u32) ^ self.seed;
                let x = k as f32 * gap + hash(id, 1) * gap * 0.6 - gap * 0.3 + if row % 2 == 1 { gap / 2.0 } else { 0.0 };
                let phase = hash(id, 2);
                let hop = if phase < 0.6 { self.jump * size * 0.9 } else { 0.0 };
                let cy = base - size * 1.6 - hop;
                // Hombros.
                for y in (cy + size * 0.7) as i32..h as i32 {
                    let half = size * 1.05;
                    for x in (x - half) as i32..=(x + half) as i32 {
                        canvas.solid(x, y, body);
                    }
                }
                // Cabeza.
                for y in (cy - size * 0.6) as i32..=(cy + size * 0.7) as i32 {
                    for xx in (x - size * 0.6) as i32..=(x + size * 0.6) as i32 {
                        let d = ((xx as f32 - x) / (size * 0.6)).powi(2) + ((y as f32 - cy) / (size * 0.7)).powi(2);
                        if d <= 1.0 {
                            let top = (y as f32) < cy - size * 0.35;
                            canvas.solid(xx, y, if top { rim } else { body });
                        }
                    }
                }
                // Manos arriba en el coro.
                if hands > 0.2 && hash(id, 3) < hands * 0.7 {
                    let side = if hash(id, 4) < 0.5 { -1.0 } else { 1.0 };
                    let wave = (self.time * 3.0 + phase * TAU).sin() * size * 0.3;
                    let hx = x + side * size * 0.9 + wave;
                    for y in (cy - size * 1.8) as i32..(cy + size * 0.8) as i32 {
                        canvas.solid(hx as i32, y, body);
                    }
                }
                // Linternas de celular en las partes tranquilas, meciéndose.
                if calm > 0.2 && hash(id, 5) < calm * 0.35 {
                    let sway = (self.time * 0.8 + phase * TAU).sin() * size * 0.6;
                    let (lx, ly) = (x + sway, cy - size * 1.7);
                    canvas.glow(lx as i32, ly as i32, Rgb(1.0, 0.97, 0.85), 0.55 * calm);
                    canvas.solid(lx as i32, ly as i32, Rgb(1.0, 0.98, 0.9));
                }
            }
        }
    }
}

impl Scene for Concert {
    fn update(&mut self, frame: &Frame, _w: usize, _h: usize) {
        self.advance(frame);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let horizon = (canvas.px_h() as f32 * FLOOR).floor().max(1.0);
        let view = View { cx: canvas.w as f32 / 2.0, ground: horizon, scale: horizon.max(4.0), weight: 1.0 };
        let scenery = &self.scenery;
        // Siempre de noche: el cielo del ánimo, oscurecido.
        canvas.clear(|_, y| scenery.sky((y as f32 / horizon).min(1.0)).scale(0.35).mix(Rgb(0.02, 0.02, 0.05), 0.5));
        self.scenery.draw_back(canvas, &view, pal);
        self.draw_stage(canvas, pal);
        self.draw_beams(canvas, pal);
        self.draw_crowd(canvas, pal);
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        for c in &self.confetti {
            let color = self.light_color(c.color as usize, pal).mix(Rgb::WHITE, 0.2);
            let (x, y) = ((c.x * w * 2.0) as i32, (c.y * h * 2.0) as i32);
            canvas.dot_over(x, y, color, 1.0);
            if c.spin.sin() > 0.0 {
                canvas.dot_over(x + 1, y, color.scale(0.8), 1.0);
            }
        }
        self.scenery.draw_weather(canvas, &view);
    }

    fn new_track(&mut self) {
        if self.blackout.is_none() {
            self.blackout = Some(0.0);
        }
        self.scenery.fade();
    }

    /// Un click: una lluvia de confeti desde ahí.
    fn poke(&mut self, x: f32, y: f32) {
        self.throw(60, Some((x, y)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(c: &mut Concert, bands: &Bands, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            c.update(&Frame { dt: 1.0 / 60.0, bands, progress: Some(0.5), song_len: 120.0, playing: true }, 120, 40);
        }
    }

    #[test]
    fn the_screen_follows_the_bands() {
        let mut c = Concert::new(1);
        run(&mut c, &Bands { bass: 1.0, level: 1.0, ..Bands::default() }, 1.0);
        assert!(c.bars[0] > 0.8 && c.bars[15] < 0.3, "{:?}", c.bars);
        run(&mut c, &Bands { treble: 1.0, level: 1.0, ..Bands::default() }, 2.0);
        assert!(c.bars[15] > 0.8 && c.bars[0] < 0.3, "{:?}", c.bars);
    }

    #[test]
    fn the_crowd_jumps_on_loud_kicks_and_a_climax_throws_confetti() {
        let mut c = Concert::new(2);
        run(&mut c, &Bands { kick: true, intensity: 0.9, ..Bands::default() }, 1.0 / 60.0);
        run(&mut c, &Bands { intensity: 0.9, ..Bands::default() }, 0.1);
        assert!(c.jump > 0.0);
        run(&mut c, &Bands::default(), 2.0);
        assert_eq!(c.jump, 0.0);
        run(&mut c, &Bands { climax: true, ..Bands::default() }, 1.0 / 60.0);
        assert!(c.confetti.len() >= 100);
    }

    #[test]
    fn beats_change_the_light_colors_and_new_songs_black_out() {
        let mut c = Concert::new(3);
        let hue = c.hue;
        run(&mut c, &Bands { beat: true, ..Bands::default() }, 1.0 / 60.0);
        assert_eq!(c.hue, hue + 1);
        run(&mut c, &Bands::default(), 1.0);
        c.new_track();
        run(&mut c, &Bands::default(), 0.5);
        assert!(c.power() < 0.5);
        run(&mut c, &Bands::default(), 2.0);
        assert!(c.blackout.is_none() && c.power() > 0.9);
    }

    #[test]
    fn draws_at_any_size() {
        let mut c = Concert::new(4);
        let bands = Bands { level: 1.0, bass: 1.0, mid: 1.0, treble: 1.0, intensity: 1.0, kick: true, beat: true, climax: true, ..Bands::default() };
        run(&mut c, &bands, 1.0);
        c.poke(0.5, 0.5);
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (220, 60)] {
            let mut canvas = Canvas::new(w, h);
            c.draw(&mut canvas, &Palette::default());
        }
    }
}

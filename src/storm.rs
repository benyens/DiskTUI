//! Escena "tormenta": un barco en alta mar, de noche, con olas grandes y relámpagos.
//!
//! La superficie del mar es una suma de ondas cuya altura crece con los graves y la
//! intensidad; el barco va encima, subiendo, bajando y escorándose según la pendiente de la
//! ola. Las crestas hacen espuma y salpican en los golpes. Los golpes fuertes y los clímax
//! traen un relámpago: un rayo quebrado que ilumina todo un instante. A lo lejos, un faro
//! gira su haz. La lluvia cae inclinada por el viento.

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// Nivel medio del mar, como fracción del alto.
const SEA: f32 = 0.60;
/// Dónde va el barco, como fracción del ancho.
const SHIP_X: f32 = 0.42;
const MAX_SPRAY: usize = 300;

/// El barco: `#` casco, `w` cabina, `y` ventana encendida, `m` mástil.
const SHIP: [&str; 7] = [
    "        m        ",
    "        m        ",
    "        m        ",
    "      wwwww      ",
    "      wywyw      ",
    "################ ",
    " ##############  ",
];

struct Bolt {
    /// Puntos del rayo, de arriba abajo, en fracciones del lienzo.
    path: Vec<(f32, f32)>,
    age: f32,
}

struct Drop {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
}

pub struct Storm {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    progress: f32,
    bands: Bands,
    /// Altura de las olas, suavizada (0 calma, 1 tormenta fuerte).
    swell: f32,
    /// Fase de las olas: avanzan con el viento.
    phase: f32,
    flash: f32,
    bolts: Vec<Bolt>,
    spray: Vec<Drop>,
    /// Segundos desde el último relámpago, para no encadenarlos.
    since_bolt: f32,
    seed: u32,
}

fn hash(a: u32, b: u32) -> f32 {
    let h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    let h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
    (h ^ (h >> 13)) as f32 / u32::MAX as f32
}

impl Storm {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0x5707_0001);
        scenery.bare = true;
        let mut rng = Rng::new(seed ^ 0x5707_A11E);
        let seed = (rng.next() * 1e6) as u32;
        Storm {
            scenery,
            rng,
            time: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            swell: 0.3,
            phase: 0.0,
            flash: 0.0,
            bolts: Vec::new(),
            spray: Vec::new(),
            since_bolt: 10.0,
            seed,
        }
    }

    /// Altura de la superficie (fracción del alto, crece hacia abajo) en `x` (fracción del
    /// ancho), con el aspecto del lienzo para que las olas no se estiren.
    fn surface(&self, x: f32, aspect: f32) -> f32 {
        let u = x * aspect;
        let a = 0.03 + 0.13 * self.swell;
        let p = self.phase;
        SEA - a * ((u * 3.1 + p).sin() * 0.6 + (u * 5.3 - p * 1.3 + 1.0).sin() * 0.3 + (u * 11.0 + p * 2.1).sin() * 0.1 * (0.5 + self.swell))
    }

    /// Un rayo quebrado desde el cielo hasta el mar, cerca de `x`.
    fn strike(&mut self, x: f32) {
        let mut path = vec![(x, 0.0)];
        let mut cx = x;
        let steps = 14;
        for k in 1..=steps {
            cx += self.rng.range(-0.035, 0.035);
            path.push((cx, SEA * k as f32 / steps as f32));
        }
        self.bolts.push(Bolt { path, age: 0.0 });
        self.flash = 1.0;
        self.since_bolt = 0.0;
    }

    fn advance(&mut self, f: &Frame, w: usize, h: usize) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }
        let target = (0.25 + 0.45 * b.intensity + 0.35 * b.bass).min(1.0) * if f.playing { 1.0 } else { 0.5 };
        self.swell += (target - self.swell) * (1.0 - (-dt / 2.0).exp());
        let tempo = if b.song.bpm > 0.0 { (b.song.bpm / 120.0).clamp(0.6, 1.5) } else { 1.0 };
        self.phase += dt * (0.6 + 0.9 * self.swell) * tempo;
        self.flash *= (-dt / 0.18).exp();
        self.since_bolt += dt;
        for bolt in &mut self.bolts {
            bolt.age += dt;
        }
        self.bolts.retain(|b| b.age < 0.5);
        // Relámpagos: en el clímax siempre; en los golpes fuertes, a veces.
        if f.playing && self.since_bolt > 0.8 && (b.climax || (b.kick && b.intensity > 0.6 && self.rng.next() < 0.25)) {
            let x = self.rng.range(0.1, 0.9);
            self.strike(x);
        }
        // Espuma que salta de las crestas en cada golpe.
        let aspect = if h > 0 { w as f32 / (h * 2) as f32 } else { 2.0 };
        if b.beat && f.playing {
            for _ in 0..(10.0 + 30.0 * self.swell) as usize {
                if self.spray.len() >= MAX_SPRAY {
                    break;
                }
                let x = self.rng.next();
                let y = self.surface(x, aspect);
                self.spray.push(Drop { x, y, vx: self.rng.range(0.02, 0.12), vy: -self.rng.range(0.05, 0.25) * (0.5 + self.swell), age: 0.0 });
            }
        }
        for d in &mut self.spray {
            d.age += dt;
            d.vy += dt * 0.6;
            d.x += d.vx * dt;
            d.y += d.vy * dt;
        }
        self.spray.retain(|d| d.age < 1.2 && d.y < 1.0);
        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    fn draw_ship(&self, canvas: &mut Canvas, aspect: f32) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let k = ((h / 28.0).floor()).clamp(1.0, 4.0);
        // Altura y pendiente de la ola donde está el barco.
        let (y, y2) = (self.surface(SHIP_X, aspect), self.surface(SHIP_X + 0.02, aspect));
        let slope = (y2 - y) * h / (0.02 * w);
        let angle = slope.atan().clamp(-0.6, 0.6);
        let (sa, ca) = angle.sin_cos();
        let (cx, cy) = (SHIP_X * w, y * h);
        let rows = SHIP.len() as f32;
        let cols = SHIP[0].len() as f32;
        let flash = self.flash;
        let hull = Rgb(0.30, 0.12, 0.08).add(Rgb::WHITE, 0.4 * flash);
        let cabin = Rgb(0.55, 0.50, 0.45).scale(0.6).add(Rgb::WHITE, 0.4 * flash);
        let mast = Rgb(0.35, 0.30, 0.25).add(Rgb::WHITE, 0.3 * flash);
        let window = Rgb(1.0, 0.80, 0.40);
        // En vez de girar el dibujo (en pixel art girar deja huecos y escalones), cada
        // columna se corre hacia arriba o abajo según la pendiente: un "cizallado". Las
        // filas del casco siguen enteras y el barco igual se inclina con la ola.
        let tilt = angle.tan();
        for (r, line) in SHIP.iter().enumerate() {
            for (c, ch) in line.bytes().enumerate() {
                let color = match ch {
                    b'#' => hull,
                    b'w' => cabin,
                    b'y' => window,
                    b'm' => mast,
                    _ => continue,
                };
                for sx in 0..k as i32 {
                    let lx = (c as f32 - cols / 2.0) * k + sx as f32;
                    let x = (cx + lx).round() as i32;
                    let y0 = cy + (r as f32 - (rows - 0.5)) * k + lx * tilt;
                    for sy in 0..k as i32 {
                        canvas.solid(x, (y0 + sy as f32).round() as i32, color);
                    }
                }
            }
        }
        // Farol en la punta del mástil, que se mece.
        let (lx, ly) = (0.0, -(rows - 0.5) * k - k);
        let (rx, ry) = (lx * ca - ly * sa, lx * sa + ly * ca);
        canvas.glow((cx + rx) as i32, (cy + ry) as i32, Rgb(1.0, 0.85, 0.5), 0.8);
    }
}

impl Scene for Storm {
    fn update(&mut self, frame: &Frame, w: usize, h: usize) {
        self.advance(frame, w, h);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        if w < 2.0 || h < 2.0 {
            return;
        }
        let aspect = w / h;
        let flash = self.flash;
        let t = self.time;
        // Cielo de tormenta: nubes en capas que corren con el viento, y el destello.
        let sky_top = Rgb(0.03, 0.04, 0.07).mix(self.scenery.sky(0.0).scale(0.3), 0.3);
        let sky_low = Rgb(0.10, 0.12, 0.16).mix(pal.accent, 0.04);
        let seed = self.seed;
        canvas.clear(|x, y| {
            let (u, v) = (x as f32 / h, y as f32 / h);
            let base = sky_top.mix(sky_low, (v / SEA).min(1.0));
            let cloud = ((u * 2.3 + t * 0.08 + (v * 6.0).sin() * 0.3).sin() * (v * 9.0 + u * 1.7 - t * 0.05).sin()).max(0.0);
            let grain = hash(x as u32 ^ seed, y as u32) * 0.02;
            base.add(Rgb(0.45, 0.48, 0.55), 0.10 * cloud + grain).add(Rgb(0.85, 0.88, 1.0), 0.55 * flash * (1.0 - v))
        });
        // Faro en una roca a la derecha, con su haz que gira.
        let (fx, fy) = (w * 0.88, h * (SEA - 0.13));
        let rock = (h * 0.03).max(2.0);
        for y in fy as i32..(h * SEA) as i32 + 4 {
            let half = (rock * (0.5 + (y as f32 - fy) / rock * 0.35)) as i32;
            for dx in -half..=half {
                canvas.solid(fx as i32 + dx, y, Rgb(0.10, 0.10, 0.11).scale(0.8 + 0.4 * hash((fx as i32 + dx) as u32, y as u32)).add(Rgb::WHITE, 0.25 * flash));
            }
        }
        let tower = (h * 0.11).max(3.0) as i32;
        for y in 0..tower {
            let stripe = if (y / 2) % 2 == 0 { Rgb(0.55, 0.12, 0.1) } else { Rgb(0.6, 0.6, 0.6) };
            canvas.solid(fx as i32, fy as i32 - y, stripe.scale(0.5 + 0.5 * flash.max(0.3)));
            canvas.solid(fx as i32 + 1, fy as i32 - y, stripe.scale(0.4 + 0.5 * flash.max(0.3)));
        }
        let beam_angle = t * 0.9;
        let (bx, by) = (fx, fy - tower as f32);
        canvas.glow(bx as i32, by as i32, Rgb(1.0, 0.95, 0.7), 0.9);
        // El haz barre de un lado a otro: un cono suave que se acorta cuando apunta hacia
        // nosotros (coseno chico) y se alarga cuando apunta de lado.
        let dir = beam_angle.cos();
        let reach = w * 0.55 * dir.abs();
        if reach > 2.0 {
            for d in 1..reach as i32 {
                let k = d as f32 / reach;
                let x = bx - dir.signum() * d as f32;
                let spread = 1.0 + d as f32 * 0.06;
                for s in -(spread as i32)..=spread as i32 {
                    let y = by + s as f32 + d as f32 * 0.03;
                    if y < self.surface(x / w, aspect) * h - 1.0 {
                        let edge = 1.0 - (s as f32 / spread).abs();
                        canvas.paint(x as i32, y as i32, Rgb(1.0, 0.95, 0.75), 0.16 * dir.abs() * (1.0 - k) * edge);
                    }
                }
            }
        }
        // Rayos.
        for bolt in &self.bolts {
            let fade = 1.0 - bolt.age / 0.5;
            for pair in bolt.path.windows(2) {
                let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
                let steps = ((y1 - y0) * h).max(1.0) as i32 * 2;
                for s in 0..=steps {
                    let k = s as f32 / steps as f32;
                    let (x, y) = ((x0 + (x1 - x0) * k) * w, (y0 + (y1 - y0) * k) * h);
                    canvas.solid(x as i32, y as i32, Rgb(0.9, 0.92, 1.0).scale(0.5 + 0.5 * fade));
                    canvas.glow(x as i32, y as i32, Rgb(0.7, 0.75, 1.0), 0.3 * fade);
                }
            }
        }
        // El barco va antes que el mar: así el agua tapa la quilla y el barco flota.
        self.draw_ship(canvas, aspect);
        // Mar: más oscuro hacia abajo, con espuma en las crestas empinadas.
        let deep = Rgb(0.02, 0.06, 0.09).mix(pal.accent, 0.03);
        let face = Rgb(0.06, 0.16, 0.20);
        for x in 0..canvas.w as i32 {
            let u = x as f32 / w;
            let top = self.surface(u, aspect) * h;
            let slope = (self.surface(u + 1.0 / w, aspect) - self.surface(u, aspect)) * h;
            // Coordenada que viaja con la ola: las motas de espuma van pegadas a ella en vez
            // de parpadear al azar (parpadear obliga a redibujar la terminal entera).
            let along = ((u * aspect * 3.1 + self.phase) * 24.0) as i32 as u32;
            for y in top.ceil() as i32..h as i32 {
                // La profundidad se mide desde el nivel medio y no desde la ola: así el agua de abajo
                // no cambia de color en cada cuadro y la terminal tiene menos que redibujar.
                let depth = ((y as f32 - (SEA - 0.12) * h) / (h * (1.12 - SEA))).clamp(0.0, 1.0);
                let mut c = face.mix(deep, depth.powf(0.5)).add(Rgb(0.8, 0.85, 1.0), 0.35 * flash * (1.0 - depth));
                // Espuma: en la cresta, más mientras más empinada y alta está la ola.
                let foam = smoothstep(0.15, 0.6, -slope) * (1.0 - (y as f32 - top) / 3.0).max(0.0);
                if foam > 0.0 || (y as f32 - top) < 1.0 && self.swell > 0.5 && hash(along ^ self.seed, 7) < 0.3 {
                    c = c.mix(Rgb(0.85, 0.9, 0.95), foam.max(0.4) * (0.5 + 0.5 * self.swell));
                }
                // Vetas de espuma arrastradas sobre la cara de la ola.
                if hash(along ^ self.seed, y as u32) < 0.025 * self.swell * (1.0 - depth) {
                    c = c.add(Rgb::WHITE, 0.15);
                }
                canvas.solid(x, y, c);
            }
        }
        // Luz de las ventanas reflejada en el agua, bajo el barco.
        let ship_y = self.surface(SHIP_X, aspect) * h;
        canvas.glow((SHIP_X * w) as i32, (ship_y + 3.0) as i32, Rgb(1.0, 0.80, 0.40), 0.2);
        // Salpicaduras.
        for d in &self.spray {
            let a = (1.0 - d.age / 1.2) * 0.8;
            canvas.paint((d.x * w) as i32, (d.y * h) as i32, Rgb(0.85, 0.9, 0.95), a);
        }
        // Lluvia inclinada por el viento: más fuerte con la tormenta.
        let drops = (w * h * (0.004 + 0.007 * self.swell)) as u32;
        let slant = 1 + (self.swell * 2.0) as i32;
        for i in 0..drops {
            let speed = 1.5 + hash(i, 2);
            let x = (hash(i, 1) * w + t * 25.0 * (0.5 + self.swell)) % w;
            let y = ((hash(i, 3) + t * speed) % 1.0) * h;
            for k in 0..3 {
                canvas.paint(x as i32 - k * slant / 2, y as i32 - k, Rgb(0.6, 0.65, 0.75), 0.25 - 0.06 * k as f32 + 0.3 * flash);
            }
        }
    }

    fn new_track(&mut self) {
        // Un trueno para empezar la canción nueva.
        let x = self.rng.range(0.2, 0.8);
        self.strike(x);
        self.scenery.fade();
    }

    /// Un click hace caer un rayo ahí.
    fn poke(&mut self, x: f32, _y: f32) {
        self.strike(x.clamp(0.02, 0.98));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &mut Storm, bands: &Bands, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            s.update(&Frame { dt: 1.0 / 60.0, bands, progress: Some(0.5), song_len: 120.0, playing: true }, 120, 34);
        }
    }

    #[test]
    fn waves_grow_with_the_music() {
        let mut calm = Storm::new(1);
        let mut rough = Storm::new(1);
        run(&mut calm, &Bands::default(), 6.0);
        run(&mut rough, &Bands { bass: 1.0, intensity: 1.0, ..Bands::default() }, 6.0);
        assert!(rough.swell > calm.swell + 0.4);
        let height = |s: &Storm| (0..100).map(|i| s.surface(i as f32 / 100.0, 2.0)).fold(f32::MIN, f32::max) - (0..100).map(|i| s.surface(i as f32 / 100.0, 2.0)).fold(f32::MAX, f32::min);
        assert!(height(&rough) > height(&calm) * 1.5);
    }

    #[test]
    fn a_climax_strikes_lightning_and_beats_throw_spray() {
        let mut s = Storm::new(2);
        run(&mut s, &Bands { climax: true, beat: true, intensity: 0.8, ..Bands::default() }, 1.0 / 60.0);
        assert_eq!(s.bolts.len(), 1);
        assert!(s.flash > 0.9);
        assert!(!s.spray.is_empty());
        // El rayo baja hasta el mar.
        let last = s.bolts[0].path.last().unwrap();
        assert!((last.1 - SEA).abs() < 1e-4);
        run(&mut s, &Bands::default(), 1.0);
        assert!(s.bolts.is_empty() && s.flash < 0.01);
    }

    #[test]
    fn a_click_strikes_where_it_was() {
        let mut s = Storm::new(3);
        s.poke(0.3, 0.2);
        assert!((s.bolts[0].path[0].0 - 0.3).abs() < 1e-4);
    }

    #[test]
    fn draws_at_any_size() {
        let mut s = Storm::new(4);
        let bands = Bands { level: 1.0, bass: 1.0, intensity: 1.0, kick: true, beat: true, climax: true, ..Bands::default() };
        run(&mut s, &bands, 1.0);
        s.poke(0.5, 0.5);
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (220, 60)] {
            let mut canvas = Canvas::new(w, h);
            s.draw(&mut canvas, &Palette::default());
        }
    }
}

//! Escena "ruta costera": un auto maneja por una ruta junto al mar.
//!
//! Todo pasa de derecha a izquierda a distintas velocidades: los cerros del horizonte casi
//! no se mueven, el mar despacio, los postes y la baranda a la velocidad del auto y las
//! flores del borde más rápido. El tempo pone la velocidad, los golpes hacen saltar el auto
//! sobre la suspensión y el clímax lo hace acelerar con líneas de velocidad. De noche se
//! encienden los faroles y los focos; con lluvia, los limpiaparabrisas no existen pero el
//! asfalto brilla.

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::render::View;
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// Horizonte del mar, comienzo y fin del asfalto, como fracciones del alto.
const HORIZON: f32 = 0.46;
const ROAD_TOP: f32 = 0.70;
const ROAD_BOTTOM: f32 = 0.84;
/// Dónde va el auto, como fracción del ancho.
const CAR_X: f32 = 0.36;
const MAX_GULLS: usize = 14;

/// El auto en píxeles (antes de escalar): `#` carrocería, `w` vidrio, `T` luz trasera,
/// `H` foco, `c` cromo. Las ruedas se dibujan aparte.
const CAR: [&str; 6] = [
    "      ######       ",
    "     #ww#www#      ",
    "    #www#wwww#     ",
    " c################ ",
    "T################H ",
    " ################c ",
];
/// Centro de las ruedas en el dibujo (columna) y su radio.
const WHEELS: [f32; 2] = [4.5, 14.0];

struct Gull {
    x: f32,
    y: f32,
    flap: f32,
    speed: f32,
}

pub struct Coast {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    progress: f32,
    bands: Bands,
    /// Distancia recorrida, en anchos de pantalla.
    distance: f32,
    speed: f32,
    /// Rebote de la suspensión (px) y su velocidad.
    bounce: f32,
    bounce_v: f32,
    climax: f32,
    gulls: Vec<Gull>,
    /// Fin de canción: el auto acelera y sale por la derecha.
    leaving: Option<f32>,
    /// Llegada de la canción nueva: el auto entra por la izquierda.
    arriving: f32,
    seed: u32,
}

fn hash(a: u32, b: u32) -> f32 {
    let h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    let h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
    (h ^ (h >> 13)) as f32 / u32::MAX as f32
}

impl Coast {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0xC0A5_7001);
        scenery.bare = true;
        let mut rng = Rng::new(seed ^ 0x0D21_7E5A);
        let seed = (rng.next() * 1e6) as u32;
        Coast {
            scenery,
            rng,
            time: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            distance: 0.0,
            speed: 0.0,
            bounce: 0.0,
            bounce_v: 0.0,
            climax: 0.0,
            gulls: Vec::new(),
            leaving: None,
            arriving: 1.0,
            seed,
        }
    }

    /// Velocidad del mundo, en anchos de pantalla por segundo.
    fn target_speed(&self, f: &Frame) -> f32 {
        if !f.playing {
            return 0.0;
        }
        let b = &self.bands;
        let tempo = if b.song.bpm > 0.0 { (b.song.bpm / 120.0).clamp(0.6, 1.6) } else { 1.0 };
        let start = smoothstep(0.0, 0.04, self.progress);
        (0.10 + 0.10 * b.intensity + 0.25 * self.climax) * tempo * (0.3 + 0.7 * start)
    }

    fn advance(&mut self, f: &Frame) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        self.climax = (self.climax * (-dt / 2.5).exp()).max(if b.climax { 1.0 } else { 0.0 });

        if let Some(t) = &mut self.leaving {
            *t += dt / 1.6;
            if *t >= 1.0 {
                self.leaving = None;
                self.arriving = 0.0;
                self.progress = 0.0;
                self.seed = (self.rng.next() * 1e6) as u32;
            }
        } else if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }
        self.arriving = (self.arriving + dt / 1.4).min(1.0);

        let target = self.target_speed(f) * if self.leaving.is_some() { 1.8 } else { 1.0 };
        self.speed += (target - self.speed) * (1.0 - (-dt / 1.2).exp());
        self.distance += self.speed * dt;

        // Suspensión: un resorte amortiguado que los golpes empujan hacia abajo.
        if b.kick && f.playing {
            self.bounce_v += 2.5 + 3.0 * b.bass;
        }
        self.bounce_v += (-self.bounce * 60.0 - self.bounce_v * 9.0) * dt;
        self.bounce = (self.bounce + self.bounce_v * dt).clamp(-1.5, 2.5);

        // Gaviotas que cruzan el cielo, algunas más de día.
        let atm = self.scenery.atmosphere();
        let rate = dt * 0.25 * atm.daylight * (1.0 - atm.rain);
        if self.gulls.len() < MAX_GULLS && self.rng.next() < rate {
            let y = self.rng.range(0.10, HORIZON - 0.12);
            for k in 0..(1 + (self.rng.next() * 3.0) as usize) {
                self.gulls.push(Gull {
                    x: 1.05 + k as f32 * 0.03,
                    y: y + k as f32 * 0.025,
                    flap: self.rng.range(0.0, 6.0),
                    speed: self.rng.range(0.02, 0.05),
                });
            }
        }
        let drift = self.speed * 0.15;
        for g in &mut self.gulls {
            g.x -= dt * (g.speed + drift);
            g.flap += dt * 5.0;
        }
        self.gulls.retain(|g| g.x > -0.1 && g.x < 1.2);

        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    /// Cerros del horizonte, el mar con su reflejo y la baranda.
    fn draw_sea(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let atm = self.scenery.atmosphere();
        let day = atm.daylight;
        let horizon = h * HORIZON;
        let road = h * ROAD_TOP;
        // Cerros lejanos: casi quietos.
        let far = self.scenery.sky(1.0).mix(Rgb(0.18, 0.24, 0.30), 0.55).scale(0.6 + 0.3 * day);
        let shift = self.distance * 0.03;
        for x in 0..canvas.w {
            let u = x as f32 / w + shift;
            let hill = (0.5 + 0.5 * (u * 5.0 + self.seed as f32 * 0.001).sin()) * (0.5 + 0.5 * (u * 2.1).cos());
            let top = horizon - h * 0.07 * hill.powf(1.5);
            for y in top as i32..horizon as i32 {
                canvas.paint(x as i32, y, far, 1.0);
            }
        }
        // Mar: más claro en el horizonte, con olas que corren despacio.
        let deep = Rgb(0.05, 0.18, 0.32).mix(pal.accent, 0.08);
        let shallow = self.scenery.sky(1.0).mix(Rgb(0.25, 0.55, 0.70), 0.5);
        let light_x = if day > 0.4 { self.scenery.sun_x() } else { self.scenery.moon_x() } * w;
        let light = if day > 0.4 { atm.sun } else { atm.moon };
        let wave_shift = self.distance * 0.25 + self.time * 0.03;
        for y in horizon as i32..road as i32 - 2 {
            let v = ((y as f32 - horizon) / (road - horizon).max(1.0)).clamp(0.0, 1.0);
            let base = shallow.mix(deep, v.powf(0.7)).scale(0.35 + 0.65 * day.max(0.25));
            for x in 0..canvas.w as i32 {
                let crest = ((x as f32 * (0.35 - 0.2 * v) + wave_shift * w * (0.2 + v)).sin() * (y as f32 * 1.7).sin()).max(0.0);
                let mut c = base.add(Rgb::WHITE, 0.06 * crest * (0.3 + v));
                // Camino de luz del sol o la luna: destellos que titilan.
                let near = (1.0 - (x as f32 - light_x).abs() / (w * (0.02 + 0.06 * v))).max(0.0);
                if near > 0.0 && hash(x as u32 ^ self.seed, (y as u32) ^ (self.time * 6.0) as u32) > 0.82 {
                    c = c.add(Rgb(1.0, 0.92, 0.75), 0.45 * near * light);
                }
                canvas.paint(x, y, c, 1.0);
            }
        }
        // Baranda: postes a la velocidad del auto y un riel continuo.
        let rail = Rgb(0.72, 0.72, 0.70).scale(0.35 + 0.6 * day.max(0.2));
        let ry = road as i32 - 2;
        for x in 0..canvas.w as i32 {
            canvas.solid(x, ry, rail);
            canvas.solid(x, ry + 1, rail.scale(0.55));
        }
        let gap = 7.0;
        let offset = (self.distance * w).rem_euclid(gap);
        let mut x = -offset;
        while x < w {
            canvas.solid(x as i32, ry - 1, rail.scale(0.8));
            x += gap;
        }
    }

    /// Faroles del lado del mar, a la velocidad del auto. De noche se encienden.
    fn draw_lamps(&self, canvas: &mut Canvas) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let atm = self.scenery.atmosphere();
        let night = 1.0 - atm.daylight;
        let gap = (w * 0.45).max(20.0);
        let offset = (self.distance * w).rem_euclid(gap);
        let base = (h * ROAD_TOP) as i32 - 2;
        let tall = (h * 0.16).max(6.0) as i32;
        let pole = Rgb(0.30, 0.30, 0.32).scale(0.5 + 0.5 * atm.daylight.max(0.3));
        let warm = Rgb(1.0, 0.82, 0.50);
        let mut x = gap * 0.6 - offset;
        while x < w + 4.0 {
            let xi = x as i32;
            for y in base - tall..base {
                canvas.solid(xi, y, pole);
            }
            for dx in 0..3 {
                canvas.solid(xi + dx, base - tall, pole);
            }
            if night > 0.2 {
                canvas.solid(xi + 2, base - tall + 1, warm.scale(0.6 + 0.4 * night));
                canvas.glow(xi + 2, base - tall + 2, warm, 0.7 * night);
                // El cono de luz en el asfalto.
                for dy in 0..(h * (ROAD_BOTTOM - ROAD_TOP)) as i32 {
                    let spread = 1 + dy / 2;
                    for dx in -spread..=spread {
                        canvas.paint(xi + 2 + dx, base + 2 + dy, warm, 0.07 * night * (1.0 - dx.abs() as f32 / (spread + 1) as f32));
                    }
                }
            }
            x += gap;
        }
    }

    fn draw_road(&self, canvas: &mut Canvas) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let atm = self.scenery.atmosphere();
        let day = atm.daylight;
        let (top, bottom) = ((h * ROAD_TOP) as i32, (h * ROAD_BOTTOM) as i32);
        let asphalt = Rgb(0.16, 0.16, 0.18).scale(0.45 + 0.55 * day.max(0.3));
        let wet = atm.rain;
        let mid = (top + bottom) / 2;
        let dash = (w * 0.06).max(4.0);
        let offset = (self.distance * w).rem_euclid(dash * 2.0);
        for y in top..bottom {
            for x in 0..canvas.w as i32 {
                let grain = 0.03 * hash(x as u32 ^ self.seed, y as u32);
                let mut c = asphalt.add(Rgb::WHITE, grain);
                // Con lluvia el asfalto refleja el cielo.
                if wet > 0.05 {
                    c = c.mix(self.scenery.sky(0.8), 0.25 * wet);
                }
                canvas.solid(x, y, c);
            }
        }
        // Borde blanco y línea al medio a trazos, que corren con la velocidad.
        let paint = Rgb(0.85, 0.85, 0.80).scale(0.5 + 0.5 * day.max(0.35));
        for x in 0..canvas.w as i32 {
            canvas.solid(x, top, paint.scale(0.8));
            let u = (x as f32 + offset).rem_euclid(dash * 2.0);
            if u < dash {
                canvas.solid(x, mid, Rgb(0.95, 0.80, 0.30).scale(0.5 + 0.5 * day.max(0.35)));
            }
        }
    }

    /// Pasto y flores del borde, lo más cercano y lo que pasa más rápido.
    fn draw_verge(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let day = self.scenery.atmosphere().daylight;
        let top = (h * ROAD_BOTTOM) as i32;
        let grass = Rgb(0.10, 0.28, 0.12).mix(pal.leaf, 0.25).scale(0.35 + 0.65 * day.max(0.2));
        let shift = self.distance * w * 1.5;
        // Estrías horizontales: el pasto pasa tan rápido que se ve barrido.
        let blur = (self.speed * 40.0).clamp(1.0, 8.0);
        for y in top..h as i32 {
            for x in 0..canvas.w as i32 {
                let u = ((x as f32 + shift) / blur) as i32;
                let tuft = hash(u.max(0) as u32 ^ self.seed, y as u32);
                let depth = (y - top) as f32 / (h - top as f32).max(1.0);
                canvas.solid(x, y, grass.scale(0.75 + 0.3 * tuft + 0.15 * depth));
            }
        }
        // Matas y flores que pasan volando.
        let flowers = [pal.petal, pal.accent, Rgb(1.0, 0.95, 0.85), Rgb(1.0, 0.80, 0.25)];
        let spacing = 3.0;
        let first = (shift / spacing).floor() as i32;
        for k in first..first + (w / spacing) as i32 + 2 {
            let x = k as f32 * spacing - shift;
            let r = hash(k as u32 ^ self.seed, 99);
            if r < 0.55 {
                continue;
            }
            let y = top as f32 + 1.0 + hash(k as u32, 7) * (h - top as f32 - 2.0);
            let tall = 1 + (hash(k as u32, 3) * 3.0) as i32;
            for dy in 0..tall {
                canvas.solid(x as i32, y as i32 - dy, grass.scale(1.3));
            }
            let c = flowers[(r * 40.0) as usize % flowers.len()].scale(0.45 + 0.55 * day.max(0.3));
            canvas.solid(x as i32, y as i32 - tall, c);
            if r > 0.8 {
                canvas.solid(x as i32 + 1, y as i32 - tall, c.scale(0.85));
            }
        }
    }

    fn draw_car(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let atm = self.scenery.atmosphere();
        let night = 1.0 - atm.daylight;
        let k = ((h / 30.0).floor() as i32).clamp(1, 4);
        let car_w = CAR[0].len() as i32 * k;
        let car_h = CAR.len() as i32 * k;
        // Entra desde la izquierda al empezar, sale por la derecha al terminar.
        let enter = smoothstep(0.0, 1.0, self.arriving);
        let leave = self.leaving.map(|t| t * t).unwrap_or(0.0);
        let x0 = (w * CAR_X - car_w as f32 / 2.0) * enter + (-(car_w as f32) - 2.0) * (1.0 - enter) + leave * w * 0.9;
        let wheel_r = (1.6 * k as f32).max(1.5);
        let ground = (h * (ROAD_TOP + ROAD_BOTTOM) / 2.0 + h * 0.035) as i32;
        let y0 = ground - car_h - wheel_r as i32 + 1 + self.bounce.round() as i32;
        let body = pal.accent.mix(Rgb(0.9, 0.2, 0.2), 0.25).scale(0.45 + 0.55 * atm.daylight.max(0.35));
        let glass = self.scenery.sky(0.3).mix(Rgb(0.6, 0.75, 0.85), 0.4).scale(0.6 + 0.4 * atm.daylight.max(0.3));
        // Sombra bajo el auto.
        for dx in 1..car_w - 1 {
            canvas.paint(x0 as i32 + dx, ground + 1, Rgb::BLACK, 0.35);
        }
        for (row, line) in CAR.iter().enumerate() {
            for (col, ch) in line.chars().enumerate() {
                let c = match ch {
                    '#' => body.scale(if row >= 4 { 0.8 } else { 1.0 }),
                    'w' => glass,
                    'T' => Rgb(0.9, 0.1, 0.1).add(Rgb::WHITE, 0.2 * night),
                    'H' => Rgb(1.0, 0.95, 0.75),
                    'c' => Rgb(0.8, 0.8, 0.82),
                    _ => continue,
                };
                for sy in 0..k {
                    for sx in 0..k {
                        canvas.solid(x0 as i32 + col as i32 * k + sx, y0 + row as i32 * k + sy, c);
                    }
                }
            }
        }
        // Ruedas: neumático oscuro y una llanta que gira.
        let spin = self.distance * w / wheel_r;
        for cx in WHEELS {
            let (wx, wy) = (x0 + cx * k as f32, (y0 + car_h) as f32 + 0.3);
            let r = wheel_r;
            for dy in -(r.ceil() as i32)..=r.ceil() as i32 {
                for dx in -(r.ceil() as i32)..=r.ceil() as i32 {
                    let d = ((dx * dx + dy * dy) as f32).sqrt();
                    if d > r + 0.3 {
                        continue;
                    }
                    let a = (dy as f32).atan2(dx as f32) + spin;
                    let hub = d < r * 0.55;
                    let c = if hub && a.sin() > 0.3 { Rgb(0.75, 0.75, 0.78) } else if hub { Rgb(0.45, 0.45, 0.48) } else { Rgb(0.06, 0.06, 0.07) };
                    canvas.solid(wx as i32 + dx, wy as i32 + dy, c);
                }
            }
        }
        // De noche: foco hacia adelante y luz trasera roja.
        if night > 0.15 {
            let hx = x0 as i32 + car_w - k;
            let hy = y0 + 4 * k;
            let reach = (w * 0.14).max(4.0);
            for d in 0..reach as i32 {
                let spread = d / 6;
                for s in -spread..=spread / 2 {
                    canvas.paint(hx + d, hy + s, Rgb(1.0, 0.88, 0.45), 0.35 * night * (1.0 - d as f32 / reach).powi(2));
                }
            }
            canvas.glow(x0 as i32, y0 + 4 * k, Rgb(1.0, 0.15, 0.1), 0.6 * night);
        }
        // Líneas de velocidad en el clímax.
        if self.climax > 0.1 {
            for i in 0..8u32 {
                let y = y0 as f32 + hash(i, self.seed) * car_h as f32;
                let len = w * 0.12 * self.climax;
                let x = x0 - 2.0 - hash(i, (self.time * 8.0) as u32) * w * 0.05;
                for d in 0..len as i32 {
                    canvas.dot_over(((x - d as f32) * 2.0) as i32, (y * 2.0) as i32, Rgb::WHITE, 0.5 * self.climax * (1.0 - d as f32 / len));
                }
            }
        }
    }

    fn draw_gulls(&self, canvas: &mut Canvas) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let c = Rgb(0.95, 0.95, 0.95).scale(0.4 + 0.6 * self.scenery.atmosphere().daylight);
        for g in &self.gulls {
            let (x, y) = ((g.x * w * 2.0) as i32, (g.y * h * 2.0) as i32);
            let up = g.flap.sin() > 0.0;
            // Una "v" de puntos: las alas suben y bajan.
            let wing = if up { -1 } else { 0 };
            canvas.dot_over(x, y, c, 1.0);
            canvas.dot_over(x - 1, y + wing, c, 1.0);
            canvas.dot_over(x + 1, y + wing, c, 1.0);
            canvas.dot_over(x - 2, y + wing * 2, c, 0.8);
            canvas.dot_over(x + 2, y + wing * 2, c, 0.8);
        }
    }
}

impl Scene for Coast {
    fn update(&mut self, frame: &Frame, _w: usize, _h: usize) {
        self.advance(frame);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let horizon = (canvas.px_h() as f32 * HORIZON).floor().max(1.0);
        let view = View { cx: canvas.w as f32 / 2.0, ground: horizon, scale: horizon.max(4.0), weight: 1.0 };
        let scenery = &self.scenery;
        canvas.clear(|_, y| scenery.sky((y as f32 / horizon).min(1.0)).mix(pal.bg_bottom, 0.08));
        self.scenery.draw_back(canvas, &view, pal);
        self.draw_gulls(canvas);
        self.draw_sea(canvas, pal);
        self.draw_lamps(canvas);
        self.draw_road(canvas);
        self.draw_car(canvas, pal);
        self.draw_verge(canvas, pal);
        self.scenery.draw_weather(canvas, &view);
    }

    fn new_track(&mut self) {
        if self.leaving.is_none() {
            self.leaving = Some(0.0);
        }
        self.scenery.fade();
    }

    /// Un click: una bandada de gaviotas cruza por ahí.
    fn poke(&mut self, x: f32, y: f32) {
        let y = y.min(HORIZON - 0.05);
        for k in 0..5 {
            if self.gulls.len() >= MAX_GULLS + 5 {
                break;
            }
            let kf = k as f32;
            self.gulls.push(Gull { x: x + 0.03 * kf, y: y + 0.02 * kf * if k % 2 == 0 { 1.0 } else { -1.0 }, flap: kf, speed: 0.06 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(c: &mut Coast, bands: &Bands, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            c.update(&Frame { dt: 1.0 / 60.0, bands, progress: Some(0.5), song_len: 120.0, playing: true }, 120, 40);
        }
    }

    #[test]
    fn drives_faster_with_intensity_and_climax() {
        let mut calm = Coast::new(1);
        let mut loud = Coast::new(1);
        run(&mut calm, &Bands::default(), 4.0);
        run(&mut loud, &Bands { intensity: 1.0, ..Bands::default() }, 4.0);
        assert!(loud.distance > calm.distance * 1.3, "{} vs {}", loud.distance, calm.distance);
        let before = loud.speed;
        run(&mut loud, &Bands { intensity: 1.0, climax: true, ..Bands::default() }, 1.0);
        assert!(loud.speed > before);
    }

    #[test]
    fn stops_when_paused_and_kicks_bounce() {
        let mut c = Coast::new(2);
        run(&mut c, &Bands::default(), 2.0);
        for _ in 0..240 {
            c.update(&Frame { dt: 1.0 / 60.0, bands: &Bands::default(), progress: Some(0.5), song_len: 120.0, playing: false }, 120, 40);
        }
        assert!(c.speed < 0.01);
        let rest = c.bounce;
        c.update(&Frame { dt: 1.0 / 60.0, bands: &Bands { kick: true, bass: 1.0, ..Bands::default() }, progress: Some(0.5), song_len: 120.0, playing: true }, 120, 40);
        c.update(&Frame { dt: 1.0 / 60.0, bands: &Bands::default(), progress: Some(0.5), song_len: 120.0, playing: true }, 120, 40);
        assert!(c.bounce > rest);
    }

    #[test]
    fn the_car_leaves_and_comes_back() {
        let mut c = Coast::new(3);
        run(&mut c, &Bands::default(), 2.0);
        c.new_track();
        run(&mut c, &Bands::default(), 2.0);
        assert!(c.leaving.is_none() && c.arriving > 0.0);
    }

    #[test]
    fn draws_at_any_size() {
        let mut c = Coast::new(4);
        let bands = Bands { level: 1.0, intensity: 1.0, climax: true, kick: true, bass: 1.0, ..Bands::default() };
        run(&mut c, &bands, 1.0);
        c.poke(0.5, 0.2);
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (220, 60)] {
            let mut canvas = Canvas::new(w, h);
            c.draw(&mut canvas, &Palette::default());
        }
    }
}

//! Escena "fuegos artificiales": un festival de verano junto a un lago, de noche.
//!
//! Los cohetes salen desde la otra orilla con el pulso, estallan con los colores de la
//! carátula y se reflejan en el agua. Dos siluetas miran desde un muelle bajo una guirnalda
//! de farolitos que se mece. Pocos fuegos al empezar, más con la intensidad, una tanda
//! grande en cada clímax y el gran final en los últimos segundos de la canción.

use std::f32::consts::TAU;

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::render::View;
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// Altura del horizonte del lago, como fracción del alto.
const HORIZON: f32 = 0.62;
/// El gran final empieza en este punto de la canción.
const FINALE_FROM: f32 = 0.90;
const MAX_SPARKS: usize = 2600;
const GRAVITY: f32 = 0.10;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    /// Esfera de estrellas.
    Peony,
    /// Esfera con colas largas.
    Chrysanthemum,
    /// Colas doradas que caen como un sauce.
    Willow,
    /// Un anillo.
    Ring,
}

/// Cohete subiendo.
struct Shell {
    x: f32,
    y: f32,
    vy: f32,
    /// Altura a la que estalla (fracción del alto).
    apex: f32,
    kind: Kind,
    size: f32,
    color: u8,
}

/// Chispa de un estallido. Posición en fracciones del ancho y del alto; `y` = 0 arriba.
struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    life: f32,
    /// Color: 0..3 de la carátula, 4 dorado de sauce.
    color: u8,
    /// Largo de la cola, en pasos.
    tail: u8,
    drag: f32,
    /// Chispa que titila al final (crepitar).
    crackle: bool,
}

pub struct Fireworks {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    progress: f32,
    bands: Bands,
    shells: Vec<Shell>,
    sparks: Vec<Spark>,
    /// Luz de los estallidos recientes sobre el cielo, el agua y la orilla.
    flash: f32,
    /// Color del último estallido (índice, como en las chispas).
    flash_color: u8,
    beat_pulse: f32,
    /// Reloj del gran final.
    finale_clock: f32,
    /// Canción nueva: el humo se despeja y el cielo queda en calma.
    clearing: Option<f32>,
}

impl Fireworks {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0x4A2A_B100);
        scenery.bare = true;
        Fireworks {
            scenery,
            rng: Rng::new(seed ^ 0x00F1_2E00),
            time: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            shells: Vec::new(),
            sparks: Vec::new(),
            flash: 0.0,
            flash_color: 0,
            beat_pulse: 0.0,
            finale_clock: 0.0,
            clearing: None,
        }
    }

    fn presence(&self) -> f32 {
        self.clearing.map(|t| 1.0 - smoothstep(0.0, 1.0, t)).unwrap_or(1.0)
    }

    fn launch(&mut self, kind: Kind, size: f32) {
        let x = self.rng.range(0.12, 0.88);
        let apex = self.rng.range(0.10, 0.34) - 0.06 * (size - 1.0);
        self.shells.push(Shell { x, y: HORIZON - 0.02, vy: -self.rng.range(0.55, 0.70), apex, kind, size, color: (self.rng.next() * 4.0) as u8 % 4 });
    }

    fn random_kind(&mut self) -> Kind {
        match (self.rng.next() * 10.0) as u32 {
            0..=4 => Kind::Peony,
            5..=6 => Kind::Chrysanthemum,
            7..=8 => Kind::Willow,
            _ => Kind::Ring,
        }
    }

    fn burst(&mut self, s: &Shell) {
        let (base, second) = (s.color % 4, (s.color + 1) % 4);
        let (count, speed, life, tail, drag) = match s.kind {
            Kind::Peony => (70, 0.30, 1.6, 1, 1.6),
            Kind::Chrysanthemum => (80, 0.32, 1.9, 4, 1.4),
            Kind::Willow => (60, 0.26, 3.2, 6, 2.2),
            Kind::Ring => (48, 0.30, 1.5, 1, 1.5),
        };
        let count = ((count as f32 * s.size) as usize).min(MAX_SPARKS.saturating_sub(self.sparks.len()));
        let tilt = self.rng.range(0.0, TAU);
        for i in 0..count {
            let angle = i as f32 / count as f32 * TAU + self.rng.range(-0.05, 0.05);
            // El anillo es plano (visto de lado se achata); los demás, esferas.
            let reach = match s.kind {
                Kind::Ring => 1.0,
                _ => self.rng.range(0.55, 1.0),
            };
            let (dx, dy) = match s.kind {
                Kind::Ring => (angle.cos(), angle.sin() * (0.35 + 0.3 * tilt.sin().abs())),
                _ => (angle.cos(), angle.sin()),
            };
            let color = match s.kind {
                Kind::Willow => 4,
                _ if i % 3 == 0 => second,
                _ => base,
            };
            self.sparks.push(Spark {
                x: s.x,
                y: s.y,
                vx: dx * speed * reach * s.size * 0.55,
                vy: dy * speed * reach * s.size,
                age: 0.0,
                life: life * self.rng.range(0.8, 1.15),
                color,
                tail,
                drag,
                crackle: s.kind != Kind::Willow && self.rng.next() < 0.25 + 0.5 * self.bands.treble,
            });
        }
        self.flash = (self.flash + 0.55 * s.size).min(1.4);
        self.flash_color = base;
    }

    fn advance(&mut self, f: &Frame) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        self.beat_pulse = (self.beat_pulse * (-dt / 0.3).exp()).max(if b.beat { 1.0 } else { 0.0 });
        self.flash *= (-dt / 0.45).exp();

        if let Some(t) = &mut self.clearing {
            *t += dt / 1.5;
            if *t >= 1.0 {
                self.clearing = None;
                self.progress = 0.0;
                self.shells.clear();
                self.sparks.clear();
            }
        } else if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }

        // Lanzamientos: con el pulso, más seguido cuanto más intensa la canción.
        if f.playing && self.clearing.is_none() && self.progress > 0.02 {
            let warm = smoothstep(0.02, 0.15, self.progress);
            if b.beat && self.rng.next() < warm * (0.18 + 0.55 * b.intensity) {
                let kind = self.random_kind();
                let size = self.rng.range(0.7, 1.0);
                self.launch(kind, size);
            }
            if b.kick && self.rng.next() < 0.10 * warm {
                self.launch(Kind::Peony, 0.55);
            }
            if b.section {
                self.launch(Kind::Ring, 1.0);
            }
            if b.climax {
                for kind in [Kind::Chrysanthemum, Kind::Peony, Kind::Willow, Kind::Peony, Kind::Chrysanthemum] {
                    let size = self.rng.range(1.0, 1.35);
                self.launch(kind, size);
                }
            }
            // Gran final: una ráfaga continua que crece hasta el último segundo.
            let finale = smoothstep(FINALE_FROM, 0.98, self.progress) * (1.0 - smoothstep(0.995, 1.0, self.progress));
            if finale > 0.0 {
                self.finale_clock += dt * (2.0 + 8.0 * finale);
                while self.finale_clock >= 1.0 {
                    self.finale_clock -= 1.0;
                    let kind = self.random_kind();
                    let size = self.rng.range(0.8, 1.3);
                self.launch(kind, size);
                }
            }
        }

        // Cohetes: suben frenando y estallan en su altura.
        let mut bursting = Vec::new();
        for s in &mut self.shells {
            s.y += s.vy * dt;
            s.vy += GRAVITY * 2.0 * dt;
            if s.y <= s.apex || s.vy >= -0.05 {
                bursting.push(Shell { kind: s.kind, ..*s });
            }
        }
        self.shells.retain(|s| !(s.y <= s.apex || s.vy >= -0.05));
        for s in bursting {
            self.burst(&s);
        }

        for p in &mut self.sparks {
            p.age += dt;
            let drag = (-p.drag * dt).exp();
            p.vx *= drag;
            p.vy = p.vy * drag + GRAVITY * dt * if p.tail > 4 { 1.4 } else { 1.0 };
            p.x += p.vx * dt;
            p.y += p.vy * dt;
        }
        self.sparks.retain(|p| p.age < p.life && p.y < HORIZON);

        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    /// Colores de los fuegos: los de la carátula, avivados, y el dorado del sauce.
    fn color(pal: &Palette, index: u8) -> Rgb {
        let vivid = |c: Rgb| c.mix(Rgb::WHITE, 0.15).scale(1.15);
        match index {
            0 => vivid(pal.petal),
            1 => vivid(pal.accent),
            2 => vivid(pal.petal_light),
            3 => vivid(pal.leaf.mix(Rgb(0.4, 0.7, 1.0), 0.5)),
            _ => Rgb(1.0, 0.78, 0.40),
        }
    }

    fn sky(&self, v: f32, pal: &Palette) -> Rgb {
        let atm = self.scenery.atmosphere();
        // Siempre de noche: con ánimos de día, apenas un resto de crepúsculo en el horizonte.
        let top = Rgb(0.02, 0.02, 0.07).mix(Rgb(0.06, 0.05, 0.16), atm.daylight * 0.5);
        let low = Rgb(0.08, 0.06, 0.18).mix(Rgb(0.42, 0.20, 0.32), atm.daylight * 0.6);
        let cloudy = atm.clouds * 0.5 + atm.rain * 0.3;
        top.mix(low, v.powf(1.6)).mix(Rgb(0.10, 0.10, 0.13), cloudy).add(Self::color(pal, self.flash_color), 0.10 * self.flash * (0.4 + 0.6 * v))
    }

    fn draw_sky(&self, canvas: &mut Canvas, hz: f32, pal: &Palette) {
        let (w, h) = (canvas.w, canvas.px_h());
        canvas.clear(|_, y| self.sky((y as f32 / hz).min(1.0), pal).mix(pal.bg_bottom, 0.06));
        // Estrellas fijas.
        let atm = self.scenery.atmosphere();
        let visible = 1.0 - 0.7 * atm.clouds - 0.4 * self.flash.min(1.0);
        for i in 0..(w as u32 * 2) {
            let hash = i.wrapping_mul(2_654_435_761);
            let (x, y) = ((hash % 997) as f32 / 997.0 * w as f32, ((hash >> 12) % 991) as f32 / 991.0 * hz * 0.85);
            let twinkle = 0.5 + 0.5 * (self.time * (0.8 + (hash % 5) as f32 * 0.3) + i as f32).sin();
            canvas.glow(x as i32, y as i32, Rgb::WHITE, 0.30 * visible * twinkle * ((hash >> 20) % 3) as f32 / 2.0);
        }
        // Cerros de la otra orilla y las luces del pueblo.
        let ridge = Rgb(0.03, 0.03, 0.07).add(Self::color(pal, self.flash_color), 0.06 * self.flash);
        for x in 0..w {
            let u = x as f32 / w as f32;
            let height = 0.07 + 0.05 * (u * 7.0 + 1.3).sin() * (u * 3.1).cos() + 0.03 * (u * 19.0).sin();
            let top = hz - height * h as f32;
            for y in top as i32..hz as i32 {
                canvas.paint(x as i32, y, ridge, 1.0);
            }
            let lot = (x as u32).wrapping_mul(2_654_435_761) >> 24;
            if lot.is_multiple_of(7) || (lot % 13 == 1 && x > 0) {
                let lamp = Rgb(1.0, 0.75, 0.45);
                canvas.paint(x as i32, hz as i32 - 1, ridge.mix(lamp, 0.7), 1.0);
            }
        }
    }

    /// Chispas y cohetes en la capa de puntos. `mirror`: dibuja el reflejo en el lago.
    fn draw_sparks(&self, canvas: &mut Canvas, hz: f32, mirror: bool, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let k = self.presence();
        let sh = h; // las fracciones de alto se miden sobre todo el lienzo
        let place = |x: f32, y: f32| -> Option<(i32, i32)> {
            let (px, py) = (x * w, y * sh);
            if !mirror {
                return (py < hz).then_some(((px * 2.0) as i32, (py * 2.0) as i32));
            }
            // Reflejo: espejado sobre el horizonte, ondulado y comprimido por la distancia.
            let depth = (hz - py) * 0.75;
            let ry = hz + depth;
            let wobble = (ry * 0.9 + self.time * 3.0).sin() * (0.5 + depth * 0.04);
            (ry < h - 1.0).then_some((((px + wobble) * 2.0) as i32, (ry * 2.0) as i32))
        };
        let strength = if mirror { 0.35 } else { 1.0 };
        for p in &self.sparks {
            let fade = 1.0 - p.age / p.life;
            let mut alpha = fade.powf(0.7) * k * strength;
            if p.crackle && fade < 0.45 {
                // Crepitar: al apagarse titila en blanco.
                let on = ((self.time * 23.0 + p.vx * 977.0).sin() > 0.2) as u8 as f32;
                alpha *= on;
            }
            if alpha < 0.03 {
                continue;
            }
            let color = if p.crackle && fade < 0.45 { Rgb(1.0, 0.98, 0.9) } else { Self::color(pal, p.color).mix(Rgb::WHITE, 0.35 * fade * fade) };
            for step in 0..=p.tail {
                let back = step as f32 * 0.035;
                let (x, y) = (p.x - p.vx * back, p.y - p.vy * back);
                if let Some((dx, dy)) = place(x, y) {
                    canvas.dot_over(dx, dy, color, alpha * (1.0 - step as f32 / (p.tail as f32 + 1.0)));
                }
            }
        }
        for s in &self.shells {
            for step in 0..4 {
                if let Some((dx, dy)) = place(s.x, s.y + step as f32 * 0.012) {
                    canvas.dot_over(dx, dy, Rgb(1.0, 0.85, 0.6), (0.9 - step as f32 * 0.22) * k * strength);
                }
            }
        }
    }

    fn draw_lake(&self, canvas: &mut Canvas, hz: f32, pal: &Palette) {
        let (w, h) = (canvas.w, canvas.px_h());
        let atm = self.scenery.atmosphere();
        let ripple = 0.6 + 1.6 * atm.wind + 1.5 * atm.rain;
        for y in hz as usize..h {
            let depth = y as f32 - hz;
            for x in 0..w {
                // El agua repite el cielo de arriba, ondulado y más oscuro.
                let wobble = ((depth * 0.8 + self.time * 2.2 + x as f32 * 0.05).sin() * ripple * (0.12 + depth * 0.02)).round() as i32;
                let src_y = (hz - 1.0 - depth * 1.33).max(0.0) as i32;
                let src = canvas.pixel(x as i32 + wobble, src_y).unwrap_or(Rgb::BLACK);
                let water = src.scale(0.75 - 0.2 * (depth / (h as f32 - hz).max(1.0))).mix(Rgb(0.03, 0.05, 0.12), 0.25);
                canvas.paint(x as i32, y as i32, water, 1.0);
            }
        }
        // Brillos en el agua cuando hay estallidos.
        if self.flash > 0.05 {
            for x in 0..w {
                for y in hz as usize..h {
                    let line = ((y as f32 * 1.7 + x as f32 * 0.21 - self.time * 4.0).sin() > 0.93) as u8 as f32;
                    canvas.glow(x as i32, y as i32, Self::color(pal, self.flash_color), 0.18 * self.flash * line);
                }
            }
        }
    }

    /// Orilla con dos siluetas sentadas mirando, y una guirnalda de farolitos arriba.
    fn draw_shore(&self, canvas: &mut Canvas, hz: f32, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        if w < 30.0 || h < 16.0 {
            return;
        }
        let flash = Self::color(pal, self.flash_color);
        let dark = Rgb(0.008, 0.008, 0.016);
        let rim = flash.mix(Rgb::WHITE, 0.4);
        let glow = self.flash.min(1.0);
        // Loma de pasto abajo a la izquierda, en primer plano.
        let bank_top = |x: f32| h - (h - hz) * (0.42 - 0.30 * (x / (w * 0.5)).powi(2));
        for x in 0..(w * 0.5) as i32 {
            let top = bank_top(x as f32);
            for y in top as i32..h as i32 {
                let edge = y == top as i32;
                canvas.solid(x, y, if edge { dark.mix(rim, 0.25 * glow) } else { dark });
            }
            // Pasto alto en el borde.
            if x % 3 == 0 {
                let blade = 1.0 + (x as f32 * 1.7).sin().abs() * 2.0;
                for k in 1..=blade as i32 {
                    canvas.solid(x, top as i32 - k, dark);
                }
            }
        }
        // Siluetas sentadas, una apoyada en la otra; el borde se ilumina con cada estallido.
        let scale = ((h - hz) / 9.0).clamp(1.5, 4.0);
        let seat_x = w * 0.22;
        for (n, x0) in [seat_x, seat_x + 4.2 * scale].into_iter().enumerate() {
            let lean = if n == 0 { 0.45 } else { -0.15 };
            let seat = bank_top(x0);
            let body = 4.2 * scale;
            for j in 0..body as i32 {
                let v = j as f32 / body;
                let half = (1.0 + 0.9 * v) * scale;
                for i in -(half as i32)..=half as i32 {
                    let x = x0 + i as f32 + lean * (1.0 - v) * scale;
                    let y = seat - body + j as f32;
                    let edge = i == -(half as i32) || j == 0;
                    canvas.solid(x as i32, y as i32, if edge { dark.mix(rim, 0.5 * glow) } else { dark });
                }
            }
            let head = 1.25 * scale;
            let (hx, hy) = (x0 + lean * scale, seat - body - head + 0.3 * scale);
            for dy in -(head as i32)..=head as i32 {
                for dx in -(head as i32)..=head as i32 {
                    let d2 = (dx * dx + dy * dy) as f32;
                    if d2 <= head * head + 0.5 {
                        let edge = dy <= 0 && d2 > (head - 1.0).powi(2);
                        let c = if edge { dark.mix(rim, 0.55 * glow) } else { dark };
                        canvas.solid((hx + dx as f32) as i32, (hy + dy as f32) as i32, c);
                    }
                }
            }
        }
        // Guirnalda de farolitos colgando de arriba; se mecen con el pulso y el viento.
        let (x_b, top, sag) = (w * 0.62, 1.0f32, hz * 0.22);
        let sway = 0.4 + 1.0 * self.beat_pulse + 2.0 * self.scenery.atmosphere().wind;
        let string = |t: f32| top + sag * 4.0 * t * (1.0 - t) * 0.9 + sag * 0.25 * t;
        for i in 0..=60 {
            let t = i as f32 / 60.0;
            canvas.dot_over((t * x_b * 2.0) as i32, (string(t) * 2.0) as i32, Rgb(0.10, 0.08, 0.08), 0.9);
        }
        let r = (0.55 * scale).max(1.0);
        for i in 1..7 {
            let t = i as f32 / 7.0;
            let swing = sway * (self.time * 1.6 + i as f32 * 0.9).sin() * 0.5;
            let (x, y) = (t * x_b + swing, string(t) + 2.0 * r);
            let lamp = [Rgb(1.0, 0.42, 0.28), Rgb(1.0, 0.72, 0.38), pal.petal.mix(Rgb(1.0, 0.55, 0.35), 0.5)][i % 3];
            let breathe = 0.85 + 0.15 * self.beat_pulse;
            for dy in -(r as i32 + 1)..=(r as i32 + 1) {
                for dx in -(r as i32)..=r as i32 {
                    let shade = 1.0 - 0.25 * (dx.abs() as f32 / (r + 0.5));
                    canvas.solid((x + dx as f32) as i32, (y + dy as f32) as i32, lamp.scale(shade * breathe));
                }
            }
            for dy in -6i32..=6 {
                for dx in -8i32..=8 {
                    let d2 = (dx * dx) as f32 / 3.0 + (dy * dy) as f32;
                    canvas.glow((x + dx as f32) as i32, (y + dy as f32) as i32, lamp, 0.09 * breathe * (-d2 / 10.0).exp());
                }
            }
        }
    }
}


impl Scene for Fireworks {
    fn update(&mut self, frame: &Frame, _w: usize, _h: usize) {
        self.advance(frame);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let hz = (canvas.px_h() as f32 * HORIZON).floor().max(1.0);
        let view = View { cx: canvas.w as f32 / 2.0, ground: hz, scale: hz.max(4.0), weight: 1.0 };
        self.draw_sky(canvas, hz, pal);
        self.draw_lake(canvas, hz, pal);
        self.draw_sparks(canvas, hz, false, pal);
        self.draw_sparks(canvas, hz, true, pal);
        self.draw_shore(canvas, hz, pal);
        self.scenery.draw_weather(canvas, &view);
    }

    fn new_track(&mut self) {
        if self.clearing.is_none() {
            self.clearing = Some(0.0);
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

    fn run(fw: &mut Fireworks, bands: &Bands, progress: f32, seconds: f32) -> usize {
        let mut launched = 0;
        for _ in 0..(seconds * 60.0) as usize {
            let before = fw.shells.len();
            fw.update(&frame(bands, progress), 120, 40);
            launched += fw.shells.len().saturating_sub(before);
        }
        launched
    }

    #[test]
    fn quiet_start_more_with_intensity_and_a_finale() {
        let beat = |intensity: f32| Bands { beat: true, intensity, ..Bands::default() };
        let mut fw = Fireworks::new(1);
        assert_eq!(run(&mut fw, &beat(1.0), 0.0, 3.0), 0, "al empezar no hay fuegos");
        let calm = run(&mut Fireworks::new(2), &beat(0.0), 0.5, 10.0);
        let loud = run(&mut Fireworks::new(2), &beat(1.0), 0.5, 10.0);
        assert!(loud > calm * 2, "la intensidad no suma fuegos: {calm} vs {loud}");
        let finale = run(&mut Fireworks::new(3), &Bands::default(), 0.97, 5.0);
        assert!(finale >= 20, "gran final flojo: {finale}");
    }

    #[test]
    fn climax_bursts_and_sparks_stay_bounded() {
        let mut fw = Fireworks::new(4);
        fw.update(&frame(&Bands { climax: true, ..Bands::default() }, 0.5), 120, 40);
        assert_eq!(fw.shells.len(), 5);
        run(&mut fw, &Bands::default(), 0.5, 2.0);
        assert!(!fw.sparks.is_empty(), "no estalló");
        run(&mut fw, &Bands { beat: true, intensity: 1.0, climax: true, ..Bands::default() }, 0.97, 20.0);
        assert!(fw.sparks.len() <= MAX_SPARKS);
    }

    #[test]
    fn new_song_clears_the_sky() {
        let mut fw = Fireworks::new(5);
        fw.update(&frame(&Bands { climax: true, ..Bands::default() }, 0.5), 120, 40);
        run(&mut fw, &Bands::default(), 0.5, 1.5);
        fw.new_track();
        run(&mut fw, &Bands::default(), 0.0, 1.6);
        assert!(fw.clearing.is_none() && fw.sparks.is_empty() && fw.shells.is_empty());
    }

    #[test]
    fn draws_at_any_size() {
        let mut fw = Fireworks::new(6);
        let bands = Bands { beat: true, intensity: 1.0, climax: true, treble: 0.8, ..Bands::default() };
        run(&mut fw, &bands, 0.95, 2.0);
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (200, 60)] {
            let mut canvas = Canvas::new(w, h);
            fw.draw(&mut canvas, &Palette::default());
        }
    }
}

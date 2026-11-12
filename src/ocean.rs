//! Escena "océano": un velero cruza el mar durante la canción y llega al faro al final.
//!
//! El cielo y el clima son los del jardín ([`Scenery`]): sol, luna, nubes, lluvia y
//! tormenta según el ánimo. El mar responde a la música: los graves levantan el oleaje, los
//! agudos lo hacen brillar, el pulso hincha las velas y los clímax hacen saltar peces.

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::render::View;
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// Altura del horizonte, como fracción del alto del lienzo.
const HORIZON: f32 = 0.56;
/// El velero zarpa en esta fracción del ancho y recorre `VOYAGE` hasta el final.
const HARBOR: f32 = 0.10;
const VOYAGE: f32 = 0.68;
/// Posición del faro, como fracción del ancho.
const LIGHTHOUSE: f32 = 0.90;

/// Pez que salta: una parábola corta sobre el agua.
struct Jump {
    /// Punto de salida, como fracción del ancho.
    x: f32,
    /// Fracción de la franja de mar donde ocurre (0 horizonte, 1 abajo).
    depth: f32,
    dir: f32,
    age: f32,
    size: f32,
}

const JUMP_TIME: f32 = 1.0;

pub struct Ocean {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    /// Reloj del oleaje: avanza más rápido con la energía y el viento.
    wave_clock: f32,
    /// Avance de la canción, 0..1.
    progress: f32,
    bands: Bands,
    /// Oleaje: sigue a los graves con algo de inercia.
    swell: f32,
    kick_pulse: f32,
    climax_pulse: f32,
    beat_pulse: f32,
    jumps: Vec<Jump>,
    /// Canción nueva: el velero se desvanece y vuelve al puerto.
    fading: Option<f32>,
}

impl Ocean {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0x0CEA_0001);
        scenery.bare = true;
        Ocean {
            scenery,
            rng: Rng::new(seed ^ 0x51A1_B0A7),
            time: 0.0,
            wave_clock: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            swell: 0.0,
            kick_pulse: 0.0,
            climax_pulse: 0.0,
            beat_pulse: 0.0,
            jumps: Vec::new(),
            fading: None,
        }
    }

    /// Posición del velero, como fracción del ancho.
    pub fn boat_x(&self) -> f32 {
        HARBOR + VOYAGE * smoothstep(0.0, 1.0, self.progress)
    }

    fn presence(&self) -> f32 {
        self.fading.map(|t| 1.0 - smoothstep(0.0, 1.0, t)).unwrap_or(1.0)
    }

    /// Altura del oleaje, en unidades de ola.
    fn amplitude(&self) -> f32 {
        let atm = self.scenery.atmosphere();
        0.5 + 2.0 * self.swell + 2.2 * atm.storm + 0.8 * atm.wind + 1.2 * self.climax_pulse
    }

    /// Desplazamiento vertical de la superficie en `x` (píxeles) y su pendiente.
    fn surface(&self, x: f32, unit: f32) -> (f32, f32) {
        let (a, t) = (self.amplitude() * unit * 0.45, self.wave_clock);
        let (k1, k2) = (0.085 / unit.max(0.5), 0.041 / unit.max(0.5));
        let (p1, p2) = (x * k1 + t * 1.6, x * k2 - t * 1.1);
        (a * (0.6 * p1.sin() + 0.4 * p2.sin()), a * (0.6 * k1 * p1.cos() + 0.4 * k2 * p2.cos()))
    }

    fn advance(&mut self, f: &Frame) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        let wind = self.scenery.atmosphere().wind;
        self.wave_clock += dt * (0.55 + 1.1 * b.level + 0.9 * wind);
        self.swell += (b.bass - self.swell) * (1.0 - (-dt / 0.28).exp());
        self.kick_pulse = (self.kick_pulse * (-dt / 0.25).exp()).max(if b.kick { 1.0 } else { 0.0 });
        self.climax_pulse = (self.climax_pulse * (-dt / 2.5).exp()).max(if b.climax { 1.0 } else { 0.0 });
        self.beat_pulse = (self.beat_pulse * (-dt / 0.30).exp()).max(if b.beat { 1.0 } else { 0.0 });

        if let Some(t) = &mut self.fading {
            *t += dt / 1.2;
            if *t >= 1.0 {
                self.fading = None;
                self.progress = 0.0;
                self.jumps.clear();
            }
        } else if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }

        // Peces: un cardumen en cada clímax, uno suelto en cada cambio de sección.
        let count = if b.climax { 3 } else if b.section { 1 } else { 0 };
        for _ in 0..count {
            let dir = if self.rng.next() < 0.5 { -1.0 } else { 1.0 };
            self.jumps.push(Jump {
                x: (self.boat_x() + self.rng.range(-0.30, 0.30)).clamp(0.05, 0.95),
                depth: self.rng.range(0.30, 0.85),
                dir,
                age: -self.rng.range(0.0, 0.5),
                size: self.rng.range(0.8, 1.3),
            });
        }
        for j in &mut self.jumps {
            j.age += dt;
        }
        self.jumps.retain(|j| j.age < JUMP_TIME);

        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    fn draw_sea(&self, canvas: &mut Canvas, hz: f32, pal: &Palette, view: &View) {
        let (w, h) = (canvas.w, canvas.px_h());
        let depth_px = (h as f32 - hz).max(1.0);
        let atm = self.scenery.atmosphere();
        let b = &self.bands;
        let horizon_sky = self.scenery.sky(1.0);
        let water = Rgb(0.03, 0.08, 0.14).mix(Rgb(0.05, 0.30, 0.46), atm.daylight).mix(pal.bg_bottom, 0.10);
        let shine = 0.05 + 0.20 * self.swell + 0.10 * atm.storm + 0.15 * self.climax_pulse;
        let lights = self.scenery.lights(canvas, view);
        let frame = (self.time * 7.0) as u32;

        for y in hz as usize..h {
            let d = ((y as f32 - hz) / depth_px).clamp(0.0, 1.0);
            // Perspectiva: las crestas se aprietan hacia el horizonte.
            let z = 10.0 * (d + 0.12).ln();
            let base = horizon_sky.mix(water, 0.45 + 0.40 * d);
            for x in 0..w {
                let u = x as f32;
                let bend = 1.3 * (u * (0.05 + 0.10 * (1.0 - d)) + self.time * 0.6 + z * 0.5).sin()
                    + 0.7 * (u * 0.21 * (1.0 - 0.5 * d) - self.time * 0.9).sin();
                let crest = (z - self.wave_clock * 1.4 + bend).sin();
                let mut color = base.scale(1.0 - 0.14 * smoothstep(0.2, 1.0, -crest));
                color = color.add(Rgb::WHITE, shine * smoothstep(0.78, 1.0, crest) * (0.25 + 0.75 * d));

                // Reflejo del sol y de la luna: una columna de luz que se abre hacia abajo.
                for (lx, radius, light, amount) in lights {
                    if amount < 0.02 {
                        continue;
                    }
                    let wobble = 2.5 * d * (z * 2.0 + self.time * 2.0).sin();
                    let spread = radius * (0.8 + 2.6 * d);
                    let fall = (-((u - lx + wobble) / spread).powi(2)).exp();
                    let ripple = 0.30 + 0.70 * crest.max(0.0);
                    color = color.add(light, amount * fall * ripple * (0.30 + 0.35 * b.level) * (1.0 - 0.5 * d));
                }

                // Destellos con los agudos.
                let hash = (x as u32).wrapping_mul(73856093) ^ (y as u32).wrapping_mul(19349663) ^ frame.wrapping_mul(83492791);
                let roll = (hash.wrapping_mul(2654435761) >> 8) as f32 / (1 << 24) as f32;
                if crest > 0.2 && roll > 0.996 - 0.014 * b.treble {
                    color = color.add(Rgb::WHITE, 0.25 + 0.45 * b.treble);
                }
                canvas.paint(x as i32, y as i32, color, 1.0);
            }
        }
    }

    /// Isla con faro. La lámpara late con el pulso y su haz barre el cielo de noche.
    fn draw_lighthouse(&self, canvas: &mut Canvas, hz: f32, pal: &Palette) {
        let atm = self.scenery.atmosphere();
        let w = canvas.w as f32;
        let x0 = w * LIGHTHOUSE;
        let night = 1.0 - 0.8 * atm.daylight;
        let land = self.scenery.sky(1.0).mix(pal.stem.mix(pal.leaf, 0.4 * atm.daylight), 0.25 + 0.35 * atm.daylight).scale(0.75);
        let (half, top) = ((w * 0.085).max(4.0), (hz * 0.07).max(1.5));
        for dx in -(half as i32)..=half as i32 {
            let t = dx as f32 / half;
            let height = top * (1.0 - t * t).max(0.0).sqrt() * (1.0 + 0.15 * (t * 5.0).sin());
            for dy in 0..height.round() as i32 {
                canvas.paint(x0 as i32 + dx, hz as i32 - 1 - dy, land, 1.0);
            }
        }

        let tower_h = (hz * 0.24).max(4.0);
        let tower_w = (hz * 0.022).clamp(1.0, 3.0);
        let base_y = hz - top;
        for i in 0..tower_h as i32 {
            let v = i as f32 / tower_h;
            // Franjas con los colores de la carátula.
            let band = if (v * 4.0) as i32 % 2 == 0 { pal.petal_light.mix(Rgb::WHITE, 0.35) } else { pal.petal };
            let color = band.scale(0.45 + 0.50 * atm.daylight + 0.10 * self.beat_pulse);
            let reach = (tower_w * (1.0 - 0.35 * v)).round() as i32;
            for dx in -reach..=reach {
                canvas.blend(x0 as i32 + dx, (base_y - i as f32) as i32, color, 1.0);
            }
        }

        // Lámpara: se enciende al zarpar y brilla más con la intensidad.
        let (lx, ly) = (x0, base_y - tower_h - 1.0);
        let lit = smoothstep(0.02, 0.12, self.progress) * (0.45 + 0.35 * self.bands.intensity + 0.40 * self.beat_pulse);
        let lamp = pal.accent.mix(Rgb::WHITE, 0.45);
        for dy in -3..=3 {
            for dx in -3..=3 {
                let d2 = (dx * dx + dy * dy) as f32;
                canvas.glow(lx as i32 + dx, ly as i32 + dy, lamp, 0.55 * lit * (-d2 / 2.5).exp());
            }
        }
        canvas.blend(lx as i32, ly as i32, lamp, lit.min(1.0));

        // Haz giratorio: se ve cuando apunta hacia los lados.
        let turn = (self.time * 0.9).cos();
        let power = lit * night * turn.abs().powf(1.5) * 0.16;
        if power > 0.004 {
            let length = w * 0.55 * turn.abs();
            for i in 2..length as i32 {
                let r = i as f32;
                let fade = power * (1.0 - r / length);
                let thick = (1.0 + r * 0.045) as i32;
                for dy in -thick..=thick {
                    let soft = 1.0 - (dy as f32 / (thick as f32 + 1.0)).abs();
                    canvas.glow((lx + turn.signum() * r) as i32, ly as i32 + dy - (r * 0.06) as i32, lamp, fade * soft);
                }
            }
        }
    }

    fn draw_boat(&self, canvas: &mut Canvas, hz: f32, pal: &Palette) {
        let k = self.presence();
        if k < 0.02 {
            return;
        }
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let depth_px = (h - hz).max(1.0);
        let unit = (depth_px / 22.0).clamp(0.8, 3.0);
        let atm = self.scenery.atmosphere();
        let bx = w * self.boat_x();
        let (lift, slope) = self.surface(bx, unit);
        let by = hz + 0.45 * depth_px + lift;
        let tilt = slope.clamp(-0.30, 0.30);
        let at = |dx: f32, dy: f32| ((bx + dx).round() as i32, (by + dy + tilt * dx).round() as i32);
        let light = 0.40 + 0.60 * atm.daylight;

        // Sombra del casco en el agua.
        let length = (6.5 * unit).round();
        for row in 0..(2.0 * unit) as i32 {
            for dx in -(length as i32)..=length as i32 {
                let (x, y) = at(dx as f32, 1.4 * unit + row as f32);
                canvas.paint(x, y, Rgb::BLACK, 0.22 * k * (1.0 - row as f32 / (2.0 * unit)));
            }
        }

        // Velas: se despliegan al zarpar y se hinchan con el pulso y el viento.
        let unfurl = smoothstep(0.0, 0.05, self.progress);
        let mast_h = 12.0 * unit;
        let mast_x = -unit;
        let billow = unit * (0.6 + 0.9 * self.beat_pulse + 1.4 * atm.wind + 0.5 * self.bands.level);
        let rows = (mast_h * 0.86 * unfurl) as i32;
        for i in 0..rows {
            let v = i as f32 / (mast_h * 0.86);
            let y = -1.6 * unit - (mast_h * 0.86) + i as f32 + (mast_h * 0.86) * (1.0 - unfurl);
            let belly = billow * (v * std::f32::consts::PI).sin();
            // Mayor: triángulo hacia la proa.
            let span = 7.0 * unit * v + belly;
            for j in 1..=span.round().max(1.0) as i32 {
                let shade = 0.80 + 0.20 * (j as f32 / span.max(1.0));
                let (x, yy) = at(mast_x + j as f32, y);
                canvas.blend(x, yy, pal.petal.mix(pal.petal_light, v * 0.6).scale(light * shade), k);
            }
            // Foque: triángulo chico hacia la popa.
            let jib = 3.6 * unit * v + 0.4 * belly;
            for j in 1..=jib.round() as i32 {
                let (x, yy) = at(mast_x - 0.6 * unit - j as f32 + belly * 0.5, y);
                canvas.blend(x, yy, pal.petal_light.mix(Rgb::WHITE, 0.3).scale(light), k);
            }
        }
        // Mástil y banderín.
        let wood = Rgb(0.62, 0.45, 0.28).scale(light);
        for i in 0..mast_h as i32 {
            let (x, y) = at(mast_x, -1.2 * unit - i as f32);
            canvas.blend(x, y, wood, k);
        }
        let flap = (self.time * 9.0).sin() * 0.5;
        for j in 1..=(2.0 * unit) as i32 {
            let (x, y) = at(mast_x + j as f32, -1.2 * unit - mast_h + 1.0 + flap * j as f32 / unit);
            canvas.blend(x, y, pal.accent.scale(0.5 + 0.5 * light), k);
        }
        // Farol: de noche, una luz cálida en lo alto.
        let lantern = k * (1.0 - atm.daylight) * (0.25 + 0.25 * self.beat_pulse);
        if lantern > 0.02 {
            let (x, y) = at(mast_x, -1.2 * unit - mast_h);
            for dy in -3..=3 {
                for dx in -3..=3 {
                    canvas.glow(x + dx, y + dy, pal.accent, lantern * (-((dx * dx + dy * dy) as f32) / 3.0).exp());
                }
            }
        }

        // Casco: trapecio con la proa a la derecha y una franja del color de la carátula.
        let hull = pal.soil.mix(Rgb(0.46, 0.28, 0.16), 0.6).scale(0.55 + 0.75 * light);
        let hull_h = (2.4 * unit).round().max(2.0) as i32;
        for row in 0..hull_h {
            let v = row as f32 / hull_h as f32;
            let (stern, bow) = (-length + v * 2.2 * unit, length + 1.5 * unit - v * 4.0 * unit);
            for dx in stern.round() as i32..=bow.round() as i32 {
                let color = if row == 0 { pal.accent.scale(0.45 + 0.55 * light) } else { hull.scale(1.0 - 0.35 * v) };
                let (x, y) = at(dx as f32, -1.2 * unit + row as f32);
                canvas.blend(x, y, color, k);
            }
        }

        // Estela y espuma en la proa con cada bombo.
        let foam = Rgb::WHITE.mix(self.scenery.sky(1.0), 0.25);
        let wake = (length * 3.0 * smoothstep(0.0, 0.08, self.progress)) as i32;
        for i in 0..wake * 2 {
            let dx = -length - i as f32 * 0.5;
            let (_, y) = at(dx, 1.2 * unit);
            let ripple = (i as f32 * 0.9 + self.time * 6.0).sin();
            let alpha = k * 0.75 * (1.0 - i as f32 / (wake * 2) as f32);
            canvas.dot_over(((bx + dx) * 2.0) as i32, y * 2 + ripple.round() as i32, foam, alpha);
        }
        let spray = self.kick_pulse * k;
        if spray > 0.08 {
            for i in 0..7 {
                let angle = 0.25 + i as f32 * 0.22;
                let reach = unit * (1.5 + 3.5 * (1.0 - self.kick_pulse));
                let (x, y) = at(length + 1.5 * unit + angle.cos() * reach, 0.8 * unit - angle.sin() * reach);
                canvas.dot_over(x * 2, y * 2, foam, spray);
            }
        }
    }

    fn draw_jumps(&self, canvas: &mut Canvas, hz: f32, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let depth_px = (h - hz).max(1.0);
        let unit = (depth_px / 22.0).clamp(0.8, 3.0);
        let k = self.presence();
        let silver = pal.accent.mix(Rgb::WHITE, 0.55);
        for j in self.jumps.iter().filter(|j| j.age >= 0.0) {
            let t = j.age / JUMP_TIME;
            let span = 7.0 * unit * j.size;
            let peak = 5.5 * unit * j.size * (0.6 + 0.4 * j.depth);
            let x = j.x * w + j.dir * span * (t - 0.5);
            let base = hz + j.depth * depth_px;
            let y = base - peak * 4.0 * t * (1.0 - t);
            // Cuerpo alineado con la trayectoria.
            let (vx, vy) = (j.dir * span, -peak * 4.0 * (1.0 - 2.0 * t));
            let norm = (vx * vx + vy * vy).sqrt().max(1e-3);
            let body = (2.2 * unit * j.size).max(1.5);
            for step in -(body as i32)..=body as i32 {
                let s = step as f32;
                let (px, py) = (x + vx / norm * s, y + vy / norm * s);
                if py < base {
                    canvas.blend(px.round() as i32, py.round() as i32, silver, k);
                    if s.abs() < body * 0.5 {
                        canvas.blend(px.round() as i32, py.round() as i32 + 1, silver.scale(0.8), k);
                    }
                }
            }
            // Salpicadura al salir y al caer.
            for (edge, at_x) in [(t, j.x * w - j.dir * span * 0.5), (1.0 - t, j.x * w + j.dir * span * 0.5)] {
                if edge < 0.25 {
                    let spread = unit * (1.0 + 10.0 * edge);
                    for i in -3i32..=3 {
                        let fx = at_x + i as f32 / 3.0 * spread;
                        let fy = base - (1.0 - (i as f32 / 3.0).powi(2)) * spread * 0.8;
                        canvas.dot_over((fx * 2.0) as i32, (fy * 2.0) as i32, Rgb::WHITE, k * (1.0 - edge * 4.0));
                    }
                }
            }
        }
    }
}

impl Scene for Ocean {
    fn update(&mut self, frame: &Frame, _w: usize, _h: usize) {
        self.advance(frame);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let ph = canvas.px_h().max(1) as f32;
        let hz = (ph * HORIZON).floor();
        let view = View { cx: canvas.w as f32 / 2.0, ground: hz, scale: hz.max(4.0), weight: 1.0 };
        let lift = 0.022 * self.bands.intensity + 0.09 * self.climax_pulse;
        let scenery = &self.scenery;
        canvas.clear(|_, y| scenery.sky((y as f32 / hz.max(1.0)).min(1.0)).mix(pal.bg_bottom, 0.12).add(pal.petal, lift));

        self.scenery.draw_back(canvas, &view, pal);
        self.draw_sea(canvas, hz, pal, &view);
        self.draw_lighthouse(canvas, hz, pal);
        self.draw_boat(canvas, hz, pal);
        self.draw_jumps(canvas, hz, pal);
        self.scenery.draw_front(canvas, &view, pal);
    }

    fn new_track(&mut self) {
        if self.fading.is_none() {
            self.fading = Some(0.0);
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
    fn boat_sails_toward_the_lighthouse() {
        let mut ocean = Ocean::new(1);
        let bands = Bands { bass: 0.5, level: 0.5, ..Bands::default() };
        ocean.update(&frame(&bands, 0.0), 120, 40);
        let start = ocean.boat_x();
        ocean.update(&frame(&bands, 0.5), 120, 40);
        let middle = ocean.boat_x();
        ocean.update(&frame(&bands, 1.0), 120, 40);
        let end = ocean.boat_x();
        assert!(start < middle && middle < end, "{start} {middle} {end}");
        assert!(end < LIGHTHOUSE, "el velero pasó el faro: {end}");
    }

    #[test]
    fn bass_raises_the_swell_and_climax_brings_fish() {
        let mut ocean = Ocean::new(2);
        let quiet = Bands::default();
        for _ in 0..60 {
            ocean.update(&frame(&quiet, 0.2), 120, 40);
        }
        let calm = ocean.amplitude();
        let loud = Bands { bass: 0.9, level: 0.8, climax: true, ..Bands::default() };
        ocean.update(&frame(&loud, 0.2), 120, 40);
        assert_eq!(ocean.jumps.len(), 3);
        let loud = Bands { climax: false, ..loud };
        for _ in 0..60 {
            ocean.update(&frame(&loud, 0.2), 120, 40);
        }
        assert!(ocean.amplitude() > calm + 1.0, "oleaje {} vs {}", ocean.amplitude(), calm);
    }

    #[test]
    fn draws_at_any_size() {
        let mut ocean = Ocean::new(3);
        let bands = Bands { bass: 0.7, treble: 0.6, level: 0.6, climax: true, kick: true, beat: true, ..Bands::default() };
        for _ in 0..30 {
            ocean.update(&frame(&bands, 0.6), 80, 24);
        }
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (200, 60)] {
            let mut canvas = Canvas::new(w, h);
            ocean.draw(&mut canvas, &Palette::default());
        }
    }

    #[test]
    fn new_song_returns_to_harbor() {
        let mut ocean = Ocean::new(4);
        let bands = Bands::default();
        ocean.update(&frame(&bands, 0.8), 120, 40);
        ocean.new_track();
        let stale = Frame { progress: Some(0.8), ..frame(&bands, 0.8) };
        for _ in 0..40 {
            ocean.update(&stale, 120, 40);
        }
        assert!(ocean.presence() < 1.0);
        for _ in 0..60 {
            ocean.update(&frame(&bands, 0.0), 120, 40);
        }
        assert!(ocean.fading.is_none() && ocean.boat_x() < HARBOR + 0.01);
    }
}

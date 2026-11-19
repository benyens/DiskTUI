//! Escena "aurora boreal": cortinas de luz sobre un lago congelado, de noche.
//!
//! La aurora se enciende al empezar la canción y se apaga al final. Los medios la hacen
//! brillar, los agudos le dan el rizado fino, los graves la mecen y los clímax la abren en
//! una corona magenta. Si el ánimo trae lluvia, aquí nieva. Una cabaña con la ventana
//! encendida late con el pulso.

use std::f32::consts::TAU;

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// Horizonte del lago, como fracción del alto.
const HORIZON: f32 = 0.66;
const MAX_FLAKES: usize = 400;
const MAX_STARS: usize = 3;

struct Flake {
    x: f32,
    y: f32,
    speed: f32,
    drift: f32,
}

/// Estrella fugaz (un click).
struct Shooting {
    x: f32,
    y: f32,
    age: f32,
}

pub struct Aurora {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    /// Reloj de la aurora: corre más rápido con el tempo.
    flow: f32,
    progress: f32,
    bands: Bands,
    glow: f32,
    corona: f32,
    beat_pulse: f32,
    flakes: Vec<Flake>,
    shooting: Vec<Shooting>,
    fading: Option<f32>,
    /// Altura de cada cortina, para variar entre canciones.
    seed: f32,
}

impl Aurora {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0xA020_0001);
        scenery.bare = true;
        let mut rng = Rng::new(seed ^ 0x0A02_02A0);
        let seed = rng.range(0.0, 100.0);
        Aurora {
            scenery,
            rng,
            time: 0.0,
            flow: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            glow: 0.0,
            corona: 0.0,
            beat_pulse: 0.0,
            flakes: Vec::new(),
            shooting: Vec::new(),
            fading: None,
            seed,
        }
    }

    fn presence(&self) -> f32 {
        self.fading.map(|t| 1.0 - smoothstep(0.0, 1.0, t)).unwrap_or(1.0)
    }

    /// Cuánto brilla la aurora: se enciende al empezar y se apaga al final.
    fn strength(&self) -> f32 {
        let on = smoothstep(0.0, 0.15, self.progress) * (1.0 - 0.7 * smoothstep(0.90, 1.0, self.progress));
        on * (0.35 + self.glow) * self.presence()
    }

    fn advance(&mut self, f: &Frame) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        let tempo = if b.song.bpm > 0.0 { (b.song.bpm / 110.0).clamp(0.6, 1.6) } else { 1.0 };
        if f.playing {
            self.flow += dt * tempo * (0.6 + 0.8 * b.level);
        }
        self.glow += (0.25 + 0.75 * b.mid - self.glow) * (1.0 - (-dt / 0.6).exp());
        self.corona = (self.corona * (-dt / 2.5).exp()).max(if b.climax { 1.0 } else { 0.0 });
        self.beat_pulse = (self.beat_pulse * (-dt / 0.3).exp()).max(if b.beat { 1.0 } else { 0.0 });

        if let Some(t) = &mut self.fading {
            *t += dt / 1.5;
            if *t >= 1.0 {
                self.fading = None;
                self.progress = 0.0;
                self.seed = self.rng.range(0.0, 100.0);
            }
        } else if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }

        // Nieve en vez de lluvia.
        let atm = self.scenery.atmosphere();
        let rate = (atm.rain * 60.0 + atm.clouds * 4.0) * dt;
        let count = rate as usize + usize::from(self.rng.next() < rate.fract());
        for _ in 0..count.min(MAX_FLAKES.saturating_sub(self.flakes.len())) {
            self.flakes.push(Flake { x: self.rng.next(), y: -0.02, speed: self.rng.range(0.03, 0.08), drift: self.rng.range(0.0, TAU) });
        }
        let wind = atm.wind;
        for fl in &mut self.flakes {
            fl.y += fl.speed * dt;
            fl.x += dt * (0.01 * (self.time * 1.3 + fl.drift).sin() + 0.04 * wind);
        }
        self.flakes.retain(|fl| fl.y < 1.0);
        for s in &mut self.shooting {
            s.age += dt;
        }
        self.shooting.retain(|s| s.age < 1.2);

        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    /// Colores de la aurora: verde clásico abajo, violeta arriba, tirados hacia la carátula
    /// y hacia el magenta en la corona.
    fn colors(&self, pal: &Palette) -> (Rgb, Rgb) {
        let low = Rgb(0.25, 1.0, 0.55).mix(pal.leaf, 0.25);
        let high = Rgb(0.55, 0.35, 1.0).mix(pal.petal, 0.30).mix(Rgb(1.0, 0.25, 0.75), 0.6 * self.corona);
        (low, high)
    }

    fn draw_sky(&self, canvas: &mut Canvas, hz: f32, pal: &Palette) {
        let (w, h) = (canvas.w, canvas.px_h() as f32);
        let atm = self.scenery.atmosphere();
        // Siempre de noche: con ánimos de día, apenas un azul más claro.
        let top = Rgb(0.01, 0.02, 0.06).mix(Rgb(0.04, 0.07, 0.16), atm.daylight * 0.6);
        let low = Rgb(0.03, 0.06, 0.12).mix(Rgb(0.12, 0.16, 0.28), atm.daylight * 0.6);
        canvas.clear(|_, y| top.mix(low, (y as f32 / hz).min(1.0)).mix(pal.bg_bottom, 0.05));
        // Estrellas.
        let visible = 1.0 - 0.8 * atm.clouds;
        for i in 0..(w as u32 * 2) {
            let hash = i.wrapping_mul(2_654_435_761);
            let (x, y) = ((hash % 997) as f32 / 997.0 * w as f32, ((hash >> 12) % 991) as f32 / 991.0 * hz * 0.9);
            let twinkle = 0.5 + 0.5 * (self.time * (0.7 + (hash % 5) as f32 * 0.25) + i as f32).sin();
            canvas.glow(x as i32, y as i32, Rgb::WHITE, 0.28 * visible * twinkle * ((hash >> 20) % 3) as f32 / 2.0);
        }
        // Cortinas: tres bandas que ondulan; rayos verticales y un borde inferior brillante.
        let power = self.strength();
        if power > 0.01 {
            let (low_c, high_c) = self.colors(pal);
            let b = &self.bands;
            let (t, sd) = (self.flow, self.seed);
            for x in 0..w {
                let u = x as f32 / w as f32;
                let sway = (0.03 + 0.05 * b.bass) * (u * 2.0 + t * 0.4).sin();
                let ripple = 0.012 * b.treble * (u * 60.0 + t * 6.0).sin();
                for (k, (base, depth, weight)) in [(0.34f32, 0.42f32, 1.0f32), (0.22, 0.30, 0.55), (0.46, 0.26, 0.40)].into_iter().enumerate() {
                    let k = k as f32;
                    // Pliegues: el borde de abajo sube y baja como una cinta.
                    let edge = base + sway + ripple + 0.09 * (u * (2.2 + 0.7 * k) + t * (0.25 + 0.1 * k) + sd + k * 2.0).sin()
                        + 0.035 * (u * 7.0 - t * 0.6 + k).sin();
                    let rays = 0.55 + 0.45 * ((u * (70.0 + 20.0 * k) + (u * 7.0 + t).sin() * 4.0).sin()).powi(2);
                    let flare = 1.0 + 1.2 * self.corona * (u * 5.0 + t * 2.0 + k).sin().max(0.0);
                    let edge_px = edge * hz * 1.3;
                    let top_px = (edge - depth * (0.6 + 0.4 * rays)) * hz * 1.3;
                    for y in top_px.max(0.0) as i32..(edge_px as i32).min(hz as i32) {
                        let v = ((y as f32 - top_px) / (edge_px - top_px).max(1.0)).clamp(0.0, 1.0);
                        // Más intensa en el borde de abajo, que se desvanece hacia arriba.
                        let fall = v.powf(2.2);
                        let color = high_c.mix(low_c, v.powf(0.6));
                        canvas.glow(x as i32, y, color, power * weight * rays * flare * fall * 0.55);
                    }
                }
            }
        }
        // Estrellas fugaces.
        for s in &self.shooting {
            let t = s.age / 1.2;
            for k in 0..10 {
                let back = k as f32 * 0.012;
                let (x, y) = ((s.x + t * 0.25 - back) * w as f32, (s.y + t * 0.10 - back * 0.4) * h);
                if y < hz {
                    canvas.dot_over((x * 2.0) as i32, (y * 2.0) as i32, Rgb::WHITE, (1.0 - t) * (1.0 - k as f32 / 10.0));
                }
            }
        }
    }

    fn draw_land(&self, canvas: &mut Canvas, hz: f32, pal: &Palette) {
        let (w, h) = (canvas.w, canvas.px_h());
        // Cerros nevados del otro lado.
        let snow = Rgb(0.12, 0.15, 0.24);
        for x in 0..w {
            let u = x as f32 / w as f32;
            let top = hz - (0.06 + 0.05 * (u * 6.0 + 0.7).sin() * (u * 2.3).cos() + 0.02 * (u * 17.0).sin()) * h as f32;
            for y in top as i32..hz as i32 {
                canvas.paint(x as i32, y, snow.scale(1.0 - 0.3 * ((y as f32 - top) / (hz - top).max(1.0))), 1.0);
            }
        }
        // Lago congelado: refleja la aurora, apagada y estirada.
        for y in hz as usize..h {
            let depth = y as f32 - hz;
            for x in 0..w {
                let src = canvas.pixel(x as i32, (hz - 1.0 - depth * 1.6).max(0.0) as i32).unwrap_or(Rgb::BLACK);
                let ice = Rgb(0.06, 0.09, 0.15).mix(src, 0.45).add(Rgb::WHITE, 0.03 * ((x as f32 * 0.7 + depth * 3.1).sin() > 0.97) as u8 as f32);
                canvas.paint(x as i32, y as i32, ice, 1.0);
            }
        }
        // Orilla con pinos y una cabaña con la ventana encendida.
        let (wf, hf) = (w as f32, h as f32);
        let shore = hz + (hf - hz) * 0.55;
        let dark = Rgb(0.01, 0.015, 0.03);
        for x in 0..(wf * 0.45) as i32 {
            let top = shore - (1.0 - x as f32 / (wf * 0.45)).powf(0.5) * (hf - hz) * 0.25;
            for y in top as i32..h as i32 {
                canvas.solid(x, y, dark);
            }
        }
        for (i, u) in [0.03f32, 0.09, 0.14, 0.31, 0.38].into_iter().enumerate() {
            let x = u * wf;
            let tall = (hf * (0.16 + 0.04 * (i % 3) as f32)).max(4.0);
            let base = shore - (1.0 - u / 0.45).powf(0.5) * (hf - hz) * 0.25;
            for j in 0..tall as i32 {
                let v = j as f32 / tall;
                let half = (v * tall * 0.28 * (0.6 + 0.4 * (v * 4.0).fract())).max(0.5);
                for dx in -(half as i32)..=half as i32 {
                    canvas.solid(x as i32 + dx, (base - tall) as i32 + j, dark);
                }
            }
        }
        let (cx, cbase) = (wf * 0.22, shore - (1.0 - 0.22f32 / 0.45).powf(0.5) * (hf - hz) * 0.25);
        let s = (hf * 0.03).clamp(1.0, 3.0);
        let cabin = Rgb(0.03, 0.025, 0.03);
        for j in 0..(4.0 * s) as i32 {
            let roof = j < (1.6 * s) as i32;
            let half = if roof { (j as f32 + 1.0) * 1.6 } else { 3.2 * s };
            for dx in -(half as i32)..=half as i32 {
                canvas.solid(cx as i32 + dx, (cbase - 4.0 * s) as i32 + j, cabin);
            }
        }
        let window = Rgb(1.0, 0.72, 0.38).mix(pal.accent, 0.2);
        let lit = self.presence() * (0.7 + 0.3 * self.beat_pulse);
        let (wx, wy) = (cx as i32 + s as i32, (cbase - 1.6 * s) as i32);
        canvas.solid(wx, wy, window.scale(lit));
        for dy in -4i32..=4 {
            for dx in -7i32..=7 {
                let d2 = (dx * dx) as f32 / 3.0 + (dy * dy) as f32;
                canvas.glow(wx + dx, wy + dy, window, 0.10 * lit * (-d2 / 7.0).exp());
            }
        }
    }
}

impl Scene for Aurora {
    fn update(&mut self, frame: &Frame, _w: usize, _h: usize) {
        self.advance(frame);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let hz = (canvas.px_h() as f32 * HORIZON).floor().max(1.0);
        self.draw_sky(canvas, hz, pal);
        self.draw_land(canvas, hz, pal);
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        for fl in &self.flakes {
            canvas.dot_over((fl.x * w * 2.0) as i32, (fl.y * h * 2.0) as i32, Rgb(0.92, 0.95, 1.0), 0.85);
        }
    }

    fn new_track(&mut self) {
        if self.fading.is_none() {
            self.fading = Some(0.0);
        }
        self.scenery.fade();
    }

    fn poke(&mut self, x: f32, y: f32) {
        if self.shooting.len() < MAX_STARS {
            self.shooting.push(Shooting { x, y: y.min(HORIZON - 0.1), age: 0.0 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(bands: &Bands, progress: f32) -> Frame<'_> {
        Frame { dt: 1.0 / 60.0, bands, progress: Some(progress), song_len: 120.0, playing: true }
    }

    fn run(a: &mut Aurora, bands: &Bands, progress: f32, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            a.update(&frame(bands, progress), 120, 40);
        }
    }

    #[test]
    fn lights_up_with_the_song_and_the_mids() {
        let mut a = Aurora::new(1);
        run(&mut a, &Bands::default(), 0.0, 1.0);
        assert!(a.strength() < 0.02, "encendida antes de tiempo");
        run(&mut a, &Bands::default(), 0.5, 3.0);
        let calm = a.strength();
        run(&mut a, &Bands { mid: 1.0, ..Bands::default() }, 0.5, 3.0);
        assert!(a.strength() > calm * 1.8, "los medios no la avivan: {calm} → {}", a.strength());
        run(&mut a, &Bands::default(), 1.0, 3.0);
        assert!(a.strength() < calm, "no se apagó al final");
    }

    #[test]
    fn rain_becomes_snow_and_stays_bounded() {
        let mut a = Aurora::new(2);
        a.scenery.force_mood(Some(crate::mood::Mood::Sad));
        run(&mut a, &Bands::default(), 0.5, 20.0);
        assert!(!a.flakes.is_empty() && a.flakes.len() <= MAX_FLAKES);
        for _ in 0..10 {
            a.poke(0.5, 0.2);
        }
        assert_eq!(a.shooting.len(), MAX_STARS);
    }

    #[test]
    fn draws_at_any_size() {
        let mut a = Aurora::new(3);
        let bands = Bands { mid: 1.0, treble: 1.0, bass: 1.0, climax: true, beat: true, ..Bands::default() };
        run(&mut a, &bands, 0.5, 1.0);
        a.poke(0.3, 0.2);
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (200, 60)] {
            let mut canvas = Canvas::new(w, h);
            a.draw(&mut canvas, &Palette::default());
        }
    }
}

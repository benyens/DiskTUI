//! Pantalla de espera: cuando no suena nada por un rato, la escena se funde en una
//! animación tranquila con los colores de la última carátula.
//!
//! Manchas de color que flotan despacio, como una lámpara de lava, y estrellas que titilan.
//! Vuelve a la escena apenas la música sigue.

use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::smoothstep;

/// Segundos sin música antes de entrar en espera.
pub const AFTER: f32 = 15.0;

#[derive(Default)]
pub struct Idle {
    time: f32,
    /// Segundos seguidos sin música.
    quiet: f32,
    /// 0 escena normal, 1 espera completa.
    level: f32,
}

impl Idle {
    pub fn update(&mut self, dt: f32, playing: bool) {
        self.time += dt;
        self.quiet = if playing { 0.0 } else { self.quiet + dt };
        // Entra despacio (3 s) y sale rápido (0,8 s) cuando vuelve la música.
        let (target, tau) = if self.quiet > AFTER { (1.0, 1.0) } else { (0.0, 0.25) };
        self.level += (target - self.level) * (1.0 - (-dt / tau).exp());
        if self.level < 0.002 {
            self.level = 0.0;
        }
    }

    pub fn level(&self) -> f32 {
        self.level
    }

    pub fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let t = self.time;
        let deep = pal.bg_top.scale(0.35);
        let colors = [pal.petal, pal.accent, pal.leaf, pal.petal_light];
        // Cuatro manchas que se mueven en curvas lentas, cada una a su ritmo.
        let blobs: Vec<(f32, f32, f32, Rgb)> = (0..4)
            .map(|i| {
                let k = i as f32;
                let x = 0.5 + 0.32 * (t * (0.031 + 0.007 * k) + k * 1.7).sin();
                let y = 0.5 + 0.28 * (t * (0.023 + 0.005 * k) + k * 2.9).cos();
                let r = 0.22 + 0.06 * (t * 0.05 + k).sin();
                (x * w, y * h, r * w.min(h * 2.0), colors[i].scale(0.75))
            })
            .collect();
        let breath = 0.85 + 0.15 * (t * std::f32::consts::TAU / 8.0).sin();
        canvas.clear(|x, y| {
            let mut c = deep;
            for (bx, by, r, color) in &blobs {
                let d = (((x as f32 - bx) / 2.0).powi(2) + (y as f32 - by).powi(2)).sqrt() / r;
                c = c.add(*color, 0.6 * breath * (1.0 - smoothstep(0.0, 1.0, d)));
            }
            c
        });
        // Estrellas: fijas, cada una titila a su ritmo.
        for i in 0..(w * h / 90.0) as u32 {
            let hash = i.wrapping_mul(2_654_435_761);
            let (x, y) = ((hash % 1000) as f32 / 1000.0 * w, ((hash >> 10) % 1000) as f32 / 1000.0 * h);
            let twinkle = 0.5 + 0.5 * (t * (0.6 + (hash % 7) as f32 * 0.15) + i as f32).sin();
            canvas.glow(x as i32, y as i32, Rgb::WHITE, 0.25 * twinkle * twinkle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_before_dimming_and_comes_back_quickly() {
        let mut idle = Idle::default();
        let dt = 1.0 / 30.0;
        for _ in 0..(AFTER * 30.0 * 0.9) as usize {
            idle.update(dt, false);
        }
        assert_eq!(idle.level(), 0.0, "entró antes de tiempo");
        for _ in 0..(6.0 * 30.0) as usize {
            idle.update(dt, false);
        }
        assert!(idle.level() > 0.95, "no entró: {}", idle.level());
        for _ in 0..30 {
            idle.update(dt, true);
        }
        assert!(idle.level() < 0.05, "no volvió rápido: {}", idle.level());
        let mut canvas = Canvas::new(40, 12);
        idle.draw(&mut canvas, &Palette::default());
        idle.draw(&mut Canvas::new(1, 1), &Palette::default());
    }
}

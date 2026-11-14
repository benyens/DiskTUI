//! El momento del drop: la escena contiene la respiración justo antes de un clímax y lo
//! suelta de golpe.
//!
//! Cuando se sabe que viene un clímax (modo archivo, que pre-analiza la canción), el reloj
//! de la escena se frena y la imagen se apaga un poco durante el último medio segundo. Al
//! llegar, un destello y el reloj acelerado un instante. Sin aviso previo (Spotify) queda
//! solo el destello.

use crate::canvas::Canvas;
use crate::color::Rgb;

/// Segundos antes del clímax en que la escena empieza a frenarse.
pub const LEAD: f32 = 0.6;

#[derive(Default)]
pub struct DropMoment {
    /// 0 normal, 1 escena casi detenida.
    tension: f32,
    /// Destello tras el clímax: 1 al llegar, baja a 0.
    release: f32,
}

impl DropMoment {
    /// `next_climax_in`: segundos hasta el próximo clímax, si se conoce.
    pub fn update(&mut self, dt: f32, next_climax_in: Option<f32>, climax: bool, playing: bool) {
        if climax {
            self.tension = 0.0;
            self.release = 1.0;
            return;
        }
        let coming = playing && next_climax_in.is_some_and(|t| t > 0.0 && t <= LEAD);
        let (target, tau) = if coming { (1.0, 0.15) } else { (0.0, 0.08) };
        self.tension += (target - self.tension) * (1.0 - (-dt / tau).exp());
        self.release *= (-dt / 0.35).exp();
    }

    /// Multiplica el paso del reloj de la escena.
    pub fn time_scale(&self) -> f32 {
        (1.0 - 0.85 * self.tension) * (1.0 + 0.8 * self.release)
    }

    /// Oscurece durante la espera y destella al soltar.
    pub fn apply(&self, canvas: &mut Canvas, flash: Rgb) {
        if self.tension > 0.01 {
            canvas.dim(1.0 - 0.28 * self.tension);
        }
        if self.release > 0.01 {
            canvas.wash(flash, 0.40 * self.release * self.release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_before_a_known_climax_and_releases_on_it() {
        let mut m = DropMoment::default();
        let dt = 1.0 / 60.0;
        m.update(dt, Some(5.0), false, true);
        assert!((m.time_scale() - 1.0).abs() < 1e-3, "lejos del clímax no frena");
        for _ in 0..30 {
            m.update(dt, Some(0.4), false, true);
        }
        assert!(m.time_scale() < 0.3, "no frenó: {}", m.time_scale());
        m.update(dt, Some(0.0), true, true);
        assert!(m.time_scale() > 1.5, "no soltó: {}", m.time_scale());
        for _ in 0..120 {
            m.update(dt, None, false, true);
        }
        assert!((m.time_scale() - 1.0).abs() < 0.01);
        // Sin música no hay espera.
        let mut paused = DropMoment::default();
        for _ in 0..30 {
            paused.update(dt, Some(0.3), false, false);
        }
        assert!((paused.time_scale() - 1.0).abs() < 1e-3);
    }
}

//! Temporizador para dormir: tecla `z` o `--dormir <minutos>`.
//!
//! Cuenta el tiempo de verdad (no solo mientras suena). En los últimos minutos baja de a
//! poco el volumen de la app y la escena se va haciendo de noche; al final pausa la música y
//! deja el volumen como estaba, para que mañana no suene bajito.

/// Opciones que recorre la tecla `z`, en minutos.
pub const CHOICES: [u32; 5] = [15, 30, 45, 60, 90];
/// Cuánto dura la bajada final, como mucho.
const FADE_MAX: f32 = 300.0;
/// El volumen baja hasta esta fracción del original antes de pausar.
const FLOOR: f32 = 0.15;

#[derive(Debug, Clone, PartialEq)]
pub struct Sleep {
    total: f32,
    left: f32,
    /// Volumen al empezar a bajar, para devolverlo al final.
    start_volume: Option<f32>,
    /// Último volumen pedido, para no mandar órdenes de más.
    sent: Option<f32>,
}

/// Lo que hay que hacer en este cuadro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    Nothing,
    SetVolume(f32),
    /// Pausar y dejar el volumen en este valor (el original), si se conoce.
    Stop(Option<f32>),
}

impl Sleep {
    pub fn new(minutes: u32) -> Self {
        let total = minutes.max(1) as f32 * 60.0;
        Sleep { total, left: total, start_volume: None, sent: None }
    }

    /// La opción que sigue a la actual: 15 → 30 → … → 90 → apagado → 15.
    pub fn next(current: Option<&Sleep>) -> Option<u32> {
        match current {
            None => Some(CHOICES[0]),
            Some(s) => {
                let minutes = (s.total / 60.0).round() as u32;
                CHOICES.iter().copied().find(|&m| m > minutes)
            }
        }
    }

    /// Minutos que faltan, redondeados hacia arriba.
    pub fn minutes_left(&self) -> u32 {
        (self.left / 60.0).ceil() as u32
    }

    /// Cuánto se bajó: 0 normal, 1 al final.
    pub fn fade(&self) -> f32 {
        let span = FADE_MAX.min(self.total * 0.5);
        (1.0 - self.left / span).clamp(0.0, 1.0)
    }

    /// Avanza el reloj. `volume`: el volumen actual de la app, si se conoce.
    pub fn tick(&mut self, dt: f32, volume: Option<f32>) -> Action {
        self.left = (self.left - dt).max(0.0);
        if self.left <= 0.0 {
            return Action::Stop(self.start_volume.or(volume));
        }
        let fade = self.fade();
        if fade <= 0.0 {
            return Action::Nothing;
        }
        let Some(start) = self.start_volume.or(volume) else { return Action::Nothing };
        self.start_volume = Some(start);
        let target = start * (1.0 - (1.0 - FLOOR) * fade);
        if self.sent.is_none_or(|s| (s - target).abs() >= 0.01) {
            self.sent = Some(target);
            return Action::SetVolume(target);
        }
        Action::Nothing
    }

    /// Hora del cielo mientras baja: del atardecer a la noche cerrada.
    pub fn sky_hour(&self) -> Option<f32> {
        let fade = self.fade();
        (fade > 0.0).then_some(19.0 + 4.5 * fade)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_cycles_through_the_choices_and_off() {
        let mut s: Option<Sleep> = None;
        let mut seen = Vec::new();
        for _ in 0..7 {
            s = Sleep::next(s.as_ref()).map(Sleep::new);
            seen.push(s.as_ref().map(|s| s.minutes_left()));
        }
        assert_eq!(seen, [Some(15), Some(30), Some(45), Some(60), Some(90), None, Some(15)]);
    }

    #[test]
    fn fades_the_volume_then_stops_and_gives_it_back() {
        let mut s = Sleep::new(15);
        // Diez minutos sin tocar nada.
        assert_eq!(s.tick(600.0, Some(0.8)), Action::Nothing);
        assert_eq!(s.fade(), 0.0);
        // Empieza a bajar.
        let mut last = 0.8;
        let mut steps = 0;
        for _ in 0..290 {
            if let Action::SetVolume(v) = s.tick(1.0, Some(last)) {
                assert!(v <= last + 1e-6);
                last = v;
                steps += 1;
            }
        }
        assert!(steps > 10 && (0.8 * FLOOR - 1e-3..0.25).contains(&last), "{steps} {last}");
        assert!(s.sky_hour().unwrap() > 23.0);
        // Al final pausa y devuelve el volumen de antes.
        assert_eq!(s.tick(20.0, Some(last)), Action::Stop(Some(0.8)));
    }

    #[test]
    fn without_volume_it_only_stops() {
        let mut s = Sleep::new(1);
        for _ in 0..59 {
            assert_eq!(s.tick(1.0, None), Action::Nothing);
        }
        assert_eq!(s.tick(1.0, None), Action::Stop(None));
    }
}

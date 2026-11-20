//! Ahorro de energía: cuántos cuadros por segundo dibujar.
//!
//! Dibujar la escena cuesta poco (1–2 ms por cuadro); lo que gasta batería es que la
//! terminal redibuje toda la pantalla 60 veces por segundo. Por eso:
//! - en pausa o en la pantalla de espera se baja a 15 FPS;
//! - con *ahorro* (o en *automática* cuando el equipo anda con batería) se baja a 30 FPS.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Energy {
    /// Fluida enchufado, ahorro con batería.
    Auto,
    /// Siempre a los FPS pedidos (salvo en pausa).
    Smooth,
    /// Siempre a 30 FPS como máximo.
    Saver,
}

/// FPS en pausa o en la pantalla de espera.
pub const PAUSED_FPS: u32 = 15;
/// FPS en ahorro.
pub const SAVER_FPS: u32 = 30;
/// Segundos en pausa antes de bajar los cuadros (para que la pausa misma se vea suave).
const PAUSE_GRACE: f32 = 2.0;

impl Energy {
    pub const ALL: [Energy; 3] = [Energy::Auto, Energy::Smooth, Energy::Saver];

    pub fn key(self) -> &'static str {
        match self {
            Energy::Auto => "auto",
            Energy::Smooth => "fluida",
            Energy::Saver => "ahorro",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Energy::ALL.into_iter().find(|e| e.key() == text.trim())
    }

    pub fn name(self) -> &'static str {
        match self {
            Energy::Auto => "ahorra con batería",
            Energy::Smooth => "siempre fluida",
            Energy::Saver => "ahorro (30 FPS)",
        }
    }

    pub fn step(self, step: i32) -> Self {
        let i = Energy::ALL.iter().position(|e| *e == self).unwrap_or(0) as i32;
        Energy::ALL[(i + step).rem_euclid(Energy::ALL.len() as i32) as usize]
    }

    /// Cuadros por segundo para este momento. `paused_for`: segundos sin reproducir.
    pub fn fps(self, wanted: u32, paused_for: f32, idle: bool, on_battery: bool) -> u32 {
        let mut fps = wanted;
        if self == Energy::Saver || (self == Energy::Auto && on_battery) {
            fps = fps.min(SAVER_FPS);
        }
        if idle || paused_for > PAUSE_GRACE {
            fps = fps.min(PAUSED_FPS);
        }
        fps.max(1)
    }
}

/// Mira cada minuto, en segundo plano, si el equipo anda con batería.
pub struct Battery {
    on: Arc<AtomicBool>,
}

impl Battery {
    pub fn watch() -> Self {
        let on = Arc::new(AtomicBool::new(false));
        let flag = on.clone();
        let _ = thread::Builder::new().name("disktui-bateria".into()).spawn(move || {
            loop {
                flag.store(on_battery(), Ordering::Relaxed);
                // Si nadie más lo mira (el programa terminó), el hilo muere con el proceso.
                thread::sleep(Duration::from_secs(60));
            }
        });
        Battery { on }
    }

    pub fn on_battery(&self) -> bool {
        self.on.load(Ordering::Relaxed)
    }
}

/// Lee `pmset -g batt` en macOS.
#[cfg(target_os = "macos")]
fn on_battery() -> bool {
    std::process::Command::new("pmset")
        .args(["-g", "batt"])
        .output()
        .ok()
        .is_some_and(|out| reports_battery(&String::from_utf8_lossy(&out.stdout)))
}

/// En Linux, alguna batería descargándose.
#[cfg(target_os = "linux")]
fn on_battery() -> bool {
    let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") else { return false };
    entries.flatten().any(|e| std::fs::read_to_string(e.path().join("status")).is_ok_and(|s| s.trim() == "Discharging"))
}

/// En Windows (y lo demás) no se detecta: *automática* se comporta como fluida.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn on_battery() -> bool {
    false
}

/// `pmset -g batt` dice "Now drawing from 'Battery Power'" con batería.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn reports_battery(pmset: &str) -> bool {
    pmset.contains("'Battery Power'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_drop_when_paused_and_on_battery() {
        assert_eq!(Energy::Auto.fps(60, 0.0, false, false), 60);
        assert_eq!(Energy::Auto.fps(60, 0.0, false, true), 30);
        assert_eq!(Energy::Smooth.fps(60, 0.0, false, true), 60);
        assert_eq!(Energy::Saver.fps(60, 0.0, false, false), 30);
        // Una pausa corta no cambia nada; una larga, sí.
        assert_eq!(Energy::Smooth.fps(60, 1.0, false, false), 60);
        assert_eq!(Energy::Smooth.fps(60, 5.0, false, false), 15);
        assert_eq!(Energy::Auto.fps(60, 0.0, true, false), 15);
        // Si se pidieron menos FPS, nunca sube.
        assert_eq!(Energy::Smooth.fps(12, 0.0, false, false), 12);
    }

    #[test]
    fn reads_pmset() {
        assert!(reports_battery("Now drawing from 'Battery Power'\n -InternalBattery-0 (id=1)\t81%; discharging;"));
        assert!(!reports_battery("Now drawing from 'AC Power'\n -InternalBattery-0 (id=1)\t100%; charged;"));
    }

    #[test]
    fn names_roundtrip_and_cycle() {
        for e in Energy::ALL {
            assert_eq!(Energy::parse(e.key()), Some(e));
        }
        assert_eq!(Energy::Auto.step(-1), Energy::Saver);
        assert_eq!(Energy::Saver.step(1), Energy::Auto);
    }
}

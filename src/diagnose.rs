//! `disktui --diagnostico`: prueba cada pieza del modo Spotify y dice cuál falla.

use std::process::Command;
use std::time::Duration;

use crate::capture::{self, CaptureOptions};
use crate::spotify;

fn command_line(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "?".into())
}

/// Imprime una línea y la guarda para el archivo de reporte.
macro_rules! say {
    ($log:ident, $($arg:tt)*) => {{
        let line = format!($($arg)*);
        println!("{line}");
        $log.push_str(&line);
        $log.push('\n');
    }};
}

/// Archivo donde queda el reporte, en la carpeta actual.
pub const REPORT_FILE: &str = "disktui-diagnostico.txt";

pub fn run(app: spotify::App, device: Option<String>) {
    let name = app.display();
    let mut log = String::new();
    say!(log, "DiskTUI · diagnóstico del modo {name}");
    say!(log, "Pon música en {name} antes de seguir: cada prueba escucha 3 segundos.\n");
    say!(log, 
        "Sistema: macOS {} · terminal: {}",
        command_line("sw_vers", &["-productVersion"]),
        std::env::var("TERM_PROGRAM").unwrap_or_else(|_| "?".into()),
    );

    say!(log, "\n[1] AppleScript → {name} (título, posición, carátula)");
    let playing = match spotify::describe_state(app) {
        Ok(state) => {
            say!(log, "    ok: {state}");
            say!(log, "    carátula → {}", spotify::describe_artwork(app));
            state.starts_with("reproduciendo")
        }
        Err(e) => {
            say!(log, "    ERROR: {e}");
            if e.contains("-1743") {
                say!(log, "    Falta permiso. Ajustes del Sistema → Privacidad y seguridad → Automatización:");
                say!(log, "    tu terminal debe poder controlar {name}.");
            } else if e.contains("syntax error") {
                say!(log, "    Es un error en el script de DiskTUI, no de permisos. Repórtalo.");
            }
            false
        }
    };
    if !playing {
        say!(log, "    AVISO: {name} no está reproduciendo. Las pruebas de audio darán silencio.");
    }

    say!(log, "\n[2] Procesos de audio que ve Core Audio");
    #[cfg(all(target_os = "macos", feature = "tap-app"))]
    match crate::tap_macos::audio_process_bundles() {
        Ok(bundles) => {
            let ours: Vec<&String> = bundles.iter().filter(|b| b.starts_with(app.bundle_prefix())).collect();
            say!(log, "    total: {}", bundles.len());
            if ours.is_empty() {
                say!(log, "    de {name}: NINGUNO. Otros: {}", bundles.join(", "));
            } else {
                say!(log, "    de {name}: {}", ours.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
            }
        }
        Err(e) => say!(log, "    ERROR: {e}"),
    }
    #[cfg(not(all(target_os = "macos", feature = "tap-app")))]
    say!(log, "    no disponible (compilado sin `tap-app` o fuera de macOS)");

    say!(log, "\n[3] Captura de audio, método por método");
    let options = CaptureOptions { device: device.clone(), global: false, bundle_prefix: app.bundle_prefix() };
    let mut working: Vec<String> = Vec::new();
    for method in capture::methods(&options) {
        say!(log, "    · {method}");
        match capture::start_method(&method, options.bundle_prefix) {
            Ok(mut cap) => {
                let (count, peak) = cap.listen(Duration::from_secs(3), false);
                let verdict = if count == 0 {
                    "SIN MUESTRAS: el stream no entrega nada"
                } else if peak <= 1e-4 {
                    "SILENCIO: llegan muestras, todas en cero (suele ser el permiso de audio)"
                } else {
                    working.push(method.to_string());
                    "OK"
                };
                say!(log, "      formato: {}", cap.format);
                say!(log, "      muestras: {count} · pico: {peak:.4} → {verdict}");
            }
            Err(e) => say!(log, "      ERROR: {e:#}"),
        }
    }

    say!(log, "\n[4] Entradas de audio disponibles (para --dispositivo)");
    let inputs = capture::input_device_names();
    if inputs.is_empty() {
        say!(log, "    ninguna");
    }
    for name in inputs {
        say!(log, "    · {name}");
    }

    say!(log, "\nResultado");
    if working.is_empty() {
        say!(log, "    Ningún método entregó audio.");
        say!(log, "    1. Ajustes del Sistema → Privacidad y seguridad → Grabación de pantalla y audio");
        say!(log, "       del sistema → sección \"Solo grabación de audio del sistema\": agrega tu");
        say!(log, "       terminal con el botón + y actívala. Cierra la terminal por completo y reábrela.");
        say!(log, "    2. Repite este diagnóstico con música sonando.");
        say!(log, "    3. Si sigue igual: BlackHole + `disktui --spotify --dispositivo BlackHole`.");
    } else {
        say!(log, "    Funciona: {}", working.join(" · "));
        say!(log, "    `disktui --spotify` elige solo el primero de esa lista que dé señal.");
    }

    match std::fs::write(REPORT_FILE, &log) {
        Ok(()) => println!("\nReporte guardado en {REPORT_FILE}"),
        Err(e) => println!("\nNo se pudo guardar {REPORT_FILE}: {e}"),
    }
}

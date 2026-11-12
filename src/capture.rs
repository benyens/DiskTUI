//! Captura de audio para el modo Spotify (vía `cpal`).
//!
//! Métodos, en orden de preferencia:
//! 1. `--dispositivo NOMBRE`: una entrada concreta (por ejemplo, BlackHole). Excluye al resto.
//! 2. Tap de Core Audio por proceso: solo el audio de la app (macOS, feature `tap-app`).
//!    Se prueban dos formas de armar el dispositivo del tap.
//! 3. Loopback global de cpal: todo lo que suena en el sistema.
//!
//! Al abrir, cada método se escucha un momento y se elige el primero que entrega señal.

use std::fmt;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use rodio::cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rodio::cpal::{self, FromSample, Sample, SampleFormat, SizedSample, SupportedStreamConfig};

use crate::analysis::{Link, PanMeter};
use crate::audio::RING_CAPACITY;
#[cfg(all(target_os = "macos", feature = "tap-app"))]
use crate::tap_macos::{Layout, ProcessTap};

/// Por debajo de este pico se considera silencio.
const SIGNAL_FLOOR: f32 = 1e-4;

pub struct CaptureOptions {
    /// Nombre (o parte) de un dispositivo de entrada.
    pub device: Option<String>,
    /// Saltarse el tap por proceso y capturar el audio global.
    pub global: bool,
    /// Prefijo del bundle id de la app a capturar.
    pub bundle_prefix: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Method {
    Device(String),
    #[cfg(all(target_os = "macos", feature = "tap-app"))]
    AppTap(Layout),
    Global,
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Method::Device(name) => write!(f, "dispositivo \"{name}\""),
            #[cfg(all(target_os = "macos", feature = "tap-app"))]
            Method::AppTap(Layout::TapOnly) => write!(f, "tap por app (solo tap)"),
            #[cfg(all(target_os = "macos", feature = "tap-app"))]
            Method::AppTap(Layout::WithOutput) => write!(f, "tap por app (con salida)"),
            Method::Global => write!(f, "audio global del sistema"),
        }
    }
}

/// Métodos a intentar para estas opciones, en orden.
pub fn methods(opts: &CaptureOptions) -> Vec<Method> {
    if let Some(name) = &opts.device {
        return vec![Method::Device(name.clone())];
    }
    let mut list = Vec::new();
    #[cfg(all(target_os = "macos", feature = "tap-app"))]
    if !opts.global {
        list.push(Method::AppTap(Layout::TapOnly));
        list.push(Method::AppTap(Layout::WithOutput));
    }
    #[cfg(not(all(target_os = "macos", feature = "tap-app")))]
    let _ = opts.global; // sin tap por app, el audio global ya es el único método
    list.push(Method::Global);
    list
}

/// Una captura en marcha. Al soltarla se detiene.
pub struct Capture {
    // El orden importa: el stream debe soltarse antes que el tap.
    _stream: cpal::Stream,
    #[cfg(all(target_os = "macos", feature = "tap-app"))]
    _tap: Option<ProcessTap>,
    /// Muestras mono capturadas. `take()` para entregarlas al hilo de análisis.
    pub consumer: Option<rtrb::Consumer<f32>>,
    /// Frecuencia de muestreo, balance estéreo y aviso de canción nueva.
    pub link: Link,
    /// De dónde viene el audio, para el HUD.
    pub label: String,
    /// Formato del stream, para el diagnóstico.
    pub format: String,
    /// Aviso sobre métodos descartados, si aplica.
    pub note: Option<String>,
}

impl Capture {
    /// Escucha hasta `max` y devuelve (muestras recibidas, pico). Con `early`, corta apenas
    /// hay señal clara.
    pub fn listen(&mut self, max: Duration, early: bool) -> (usize, f32) {
        let start = Instant::now();
        let (mut count, mut peak) = (0usize, 0.0f32);
        while start.elapsed() < max {
            std::thread::sleep(Duration::from_millis(40));
            if let Some(consumer) = self.consumer.as_mut() {
                while let Ok(s) = consumer.pop() {
                    count += 1;
                    peak = peak.max(s.abs());
                }
            }
            if early && peak > SIGNAL_FLOOR && count > 2000 {
                break;
            }
        }
        (count, peak)
    }
}

/// Abre la captura con el primer método que entregue señal.
pub fn open(opts: &CaptureOptions) -> Result<Capture> {
    let list = methods(opts);
    let mut silent: Vec<Method> = Vec::new();
    let mut failed: Vec<String> = Vec::new();

    for method in &list {
        match start_method(method, opts.bundle_prefix) {
            Ok(mut capture) => {
                // Con un solo método no hay nada que comparar.
                if list.len() == 1 {
                    return Ok(capture);
                }
                let (_, peak) = capture.listen(Duration::from_millis(1200), true);
                if peak > SIGNAL_FLOOR {
                    if !silent.is_empty() || !failed.is_empty() {
                        capture.note = Some(format!("usando {method}"));
                    }
                    return Ok(capture);
                }
                silent.push(method.clone());
            }
            Err(e) => failed.push(format!("{method}: {e:#}")),
        }
    }

    // Ninguno dio señal (¿música en pausa?, ¿falta el permiso?): queda el preferido.
    if let Some(method) = silent.first() {
        let mut capture = start_method(method, opts.bundle_prefix)?;
        capture.note = Some("sin señal al iniciar; prueba `disktui --diagnostico`".into());
        return Ok(capture);
    }
    bail!("no se pudo capturar audio. {}", failed.join(" · "))
}

/// Abre un método concreto y arranca el stream.
pub fn start_method(method: &Method, bundle_prefix: &str) -> Result<Capture> {
    let host = cpal::default_host();
    #[cfg(not(all(target_os = "macos", feature = "tap-app")))]
    let _ = bundle_prefix;

    match method {
        Method::Device(wanted) => {
            let needle = wanted.to_lowercase();
            let device = host
                .input_devices()
                .context("no se pudieron listar los dispositivos de entrada")?
                .find(|d| device_name(d).to_lowercase().contains(&needle))
                .ok_or_else(|| anyhow!("no hay dispositivo de entrada que contenga \"{wanted}\""))?;
            let config = device.default_input_config().context("el dispositivo no tiene formato de entrada")?;
            let label = device_name(&device);
            start(&device, config, label)
        }
        #[cfg(all(target_os = "macos", feature = "tap-app"))]
        Method::AppTap(layout) => {
            let tap = ProcessTap::for_bundle_prefix(bundle_prefix, *layout).map_err(|e| anyhow!(e))?;
            // El tap vive en un dispositivo agregado privado; cpal lo abre como una entrada.
            let id = cpal::DeviceId::new(host.id(), &tap.device_uid);
            let device = host
                .device_by_id(&id)
                .ok_or_else(|| anyhow!("cpal no ve el dispositivo del tap"))?;
            if !device.supports_input() {
                bail!("el dispositivo del tap no expone entrada");
            }
            let config = device.default_input_config().context("el tap no tiene formato de entrada")?;
            let mut capture = start(&device, config, "solo la app".into())?;
            capture._tap = Some(tap);
            Ok(capture)
        }
        Method::Global => {
            // cpal activa el loopback al pedir entrada sobre un dispositivo de salida.
            let device = host.default_output_device().ok_or_else(|| anyhow!("no hay dispositivo de salida"))?;
            let config = if device.supports_input() {
                device.default_input_config()
            } else {
                device.default_output_config()
            }
            .context("no se pudo leer el formato del dispositivo de salida")?;
            start(&device, config, "audio del sistema".into())
        }
    }
}

/// Nombres de los dispositivos de entrada. Para el diagnóstico.
pub fn input_device_names() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|devices| devices.map(|d| device_name(&d)).collect())
        .unwrap_or_default()
}

fn device_name(device: &cpal::Device) -> String {
    device.description().map(|d| d.name().to_string()).unwrap_or_else(|_| "?".into())
}

/// Abre el stream de entrada con el formato nativo del dispositivo y lo arranca.
fn start(device: &cpal::Device, config: SupportedStreamConfig, label: String) -> Result<Capture> {
    let (producer, consumer) = rtrb::RingBuffer::new(RING_CAPACITY);
    let link = Link::new(config.sample_rate());
    let format = format!(
        "{} Hz, {} canales, {:?}",
        config.sample_rate(),
        config.channels(),
        config.sample_format()
    );
    let stream = match config.sample_format() {
        SampleFormat::F32 => build::<f32>(device, &config, producer, link.clone()),
        SampleFormat::I16 => build::<i16>(device, &config, producer, link.clone()),
        SampleFormat::I32 => build::<i32>(device, &config, producer, link.clone()),
        SampleFormat::U16 => build::<u16>(device, &config, producer, link.clone()),
        other => bail!("formato de muestra no soportado: {other:?}"),
    }?;
    stream.play().context("no se pudo iniciar la captura")?;
    Ok(Capture {
        _stream: stream,
        #[cfg(all(target_os = "macos", feature = "tap-app"))]
        _tap: None,
        consumer: Some(consumer),
        link,
        label,
        format,
        note: None,
    })
}

fn build<T>(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    mut producer: rtrb::Producer<f32>,
    link: Link,
) -> Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels().max(1) as usize;
    let mut pan = PanMeter::default();
    let stream = device.build_input_stream(
        config.config(),
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            // Mezcla a mono: promedio de los canales de cada cuadro.
            for frame in data.chunks_exact(channels) {
                let sum: f32 = frame.iter().map(|s| f32::from_sample(*s)).sum();
                let _ = producer.push(sum / channels as f32);
                if channels >= 2 {
                    pan.frame(f32::from_sample(frame[0]), f32::from_sample(frame[1]), &link);
                }
            }
        },
        // Los errores del stream no se imprimen: romperían la pantalla de la TUI.
        |_err| {},
        None,
    )?;
    Ok(stream)
}

//! Modo archivo: reproducción con `rodio` y un "tap" que copia las muestras al análisis.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use anyhow::{Context, Result};
use rodio::source::SeekError;
use rodio::{ChannelCount, Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Sample, SampleRate, Source};

use crate::analysis::{self, Bands, Link, PanMeter};
use crate::meta;
use crate::palette::{Cover, Palette};
use crate::session::{Session, Tick, TrackInfo, drain_bands};
use crate::structure::SongMap;

/// Capacidad del ring buffer entre el audio y el análisis: más de un segundo de audio.
pub const RING_CAPACITY: usize = 1 << 17;

/// Adaptador de `Source`: deja pasar el audio intacto y, de paso, empuja una copia mono al
/// ring buffer. Como rodio pide las muestras justo antes de reproducirlas, el análisis queda
/// sincronizado con lo que suena.
pub struct Tap<S> {
    inner: S,
    out: rtrb::Producer<f32>,
    link: Link,
    channels: u16,
    index: u16,
    acc: f32,
    first: f32,
    pan: PanMeter,
}

impl<S: Source> Tap<S> {
    pub fn new(inner: S, out: rtrb::Producer<f32>, link: Link) -> Self {
        let channels = inner.channels().get();
        link.sample_rate.store(inner.sample_rate().get(), Ordering::Relaxed);
        Tap { inner, out, link, channels, index: 0, acc: 0.0, first: 0.0, pan: PanMeter::default() }
    }
}

impl<S: Source> Iterator for Tap<S> {
    type Item = Sample;

    #[inline]
    fn next(&mut self) -> Option<Sample> {
        let s = self.inner.next()?;
        self.acc += s;
        if self.index == 0 {
            self.first = s;
        } else if self.index == 1 {
            // Estéreo: canal 0 es izquierda, canal 1 es derecha.
            self.pan.frame(self.first, s, &self.link);
        }
        self.index += 1;
        if self.index >= self.channels {
            // Fin del cuadro: promedia los canales. Si el ring está lleno, se descarta.
            let _ = self.out.push(self.acc / self.channels as f32);
            self.acc = 0.0;
            self.index = 0;
            // El formato puede cambiar a mitad de archivo; se vuelve a leer en cada cuadro.
            self.channels = self.inner.channels().get();
            self.link.sample_rate.store(self.inner.sample_rate().get(), Ordering::Relaxed);
        }
        Some(s)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<S: Source> Source for Tap<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }
    fn channels(&self) -> ChannelCount {
        self.inner.channels()
    }
    fn sample_rate(&self) -> SampleRate {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(pos)
    }
}

pub struct FileSession {
    // El orden importa: `player` debe soltarse antes que `_sink`.
    player: Player,
    _sink: MixerDeviceSink,
    rx: Receiver<Bands>,
    last: Bands,
    stop: Arc<AtomicBool>,
    info: TrackInfo,
    palette: Option<Palette>,
    cover: Option<Cover>,
    /// Llega cuando termina el pre-análisis del archivo.
    map_rx: Receiver<SongMap>,
    map: Option<SongMap>,
    last_pos: f32,
}

impl FileSession {
    pub fn open(path: &Path) -> Result<Self> {
        let (mut info, cover) = meta::read(path);
        let palette = cover.as_deref().and_then(Palette::from_image_bytes);
        let thumbnail = cover.as_deref().and_then(Cover::from_image_bytes);

        let file = File::open(path).with_context(|| format!("no se pudo abrir {}", path.display()))?;
        let decoder = Decoder::try_from(file)
            .with_context(|| format!("formato no soportado: {}", path.display()))?;
        if info.duration.is_none() {
            info.duration = decoder.total_duration();
        }

        let mut sink = DeviceSinkBuilder::open_default_sink()
            .context("no hay dispositivo de salida de audio")?;
        sink.log_on_drop(false);
        let player = Player::connect_new(sink.mixer());

        let (producer, consumer) = rtrb::RingBuffer::new(RING_CAPACITY);
        let link = Link::new(44_100);
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        analysis::spawn(consumer, link.clone(), tx, stop.clone());

        player.append(Tap::new(decoder, producer, link));

        // Pre-análisis de toda la canción en otro hilo: ubica los clímax de antemano.
        let (map_tx, map_rx) = mpsc::channel();
        let map_path = path.to_path_buf();
        std::thread::spawn(move || {
            if let Some(map) = SongMap::analyze_file(&map_path) {
                let _ = map_tx.send(map);
            }
        });

        Ok(FileSession {
            player,
            _sink: sink,
            rx,
            last: Bands::default(),
            stop,
            info,
            // Sin carátula legible se usa la paleta fija.
            palette: Some(palette.unwrap_or_default()),
            cover: thumbnail,
            map_rx,
            map: None,
            last_pos: 0.0,
        })
    }
}

impl Session for FileSession {
    fn tick(&mut self, _dt: f32) -> Tick {
        let mut bands = drain_bands(&self.rx, &mut self.last);
        if let Ok(map) = self.map_rx.try_recv() {
            self.map = Some(map);
        }
        // Con el mapa listo, la intensidad y los clímax salen de la canción completa y
        // reemplazan a la estimación en tiempo real.
        if let Some(map) = &self.map {
            let pos = self.player.get_pos().as_secs_f32();
            bands.intensity = map.intensity_at(pos);
            bands.climax = map.climax_between(self.last_pos, pos);
            bands.section = bands.climax || map.section_between(self.last_pos, pos);
            // Tempo, tonalidad y ánimo de la canción entera, conocidos desde el inicio. El
            // pulso (cuándo cae cada golpe) sigue viniendo del análisis en vivo.
            bands.song = map.song;
            self.last_pos = pos;
        }
        Tick { bands, new_track: false, palette: self.palette.take(), cover: self.cover.take() }
    }
    fn track(&self) -> &TrackInfo {
        &self.info
    }
    fn position(&self) -> Duration {
        self.player.get_pos()
    }
    fn playing(&self) -> bool {
        !self.player.is_paused() && !self.player.empty()
    }
    fn finished(&self) -> bool {
        self.player.empty()
    }
    fn toggle_pause(&mut self) {
        if self.player.is_paused() {
            self.player.play();
        } else {
            self.player.pause();
        }
    }
    fn change_volume(&mut self, steps: i32) {
        let v = (self.player.volume() + steps as f32 * 0.05).clamp(0.0, 1.5);
        self.player.set_volume(v);
    }
    fn volume(&self) -> Option<f32> {
        Some(self.player.volume())
    }
    fn label(&self) -> String {
        "archivo".into()
    }
    fn song_map(&self) -> Option<&SongMap> {
        self.map.as_ref()
    }
}

impl Drop for FileSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.player.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{ANALYSIS_HZ, Analyzer};
    use rodio::source::SineWave;

    #[test]
    fn tap_passes_audio_through_and_copies_mono() {
        let (producer, mut consumer) = rtrb::RingBuffer::new(RING_CAPACITY);
        let link = Link::new(0);
        let sr = link.sample_rate.clone();
        let source = SineWave::new(1000.0).take_duration(Duration::from_millis(500));
        let expected: Vec<f32> = SineWave::new(1000.0).take_duration(Duration::from_millis(500)).collect();
        let tapped: Vec<f32> = Tap::new(source, producer, link).collect();
        assert_eq!(tapped, expected, "el tap no debe alterar el audio");
        assert_eq!(sr.load(Ordering::Relaxed), 48_000);
        let mut copied = Vec::new();
        while let Ok(s) = consumer.pop() {
            copied.push(s);
        }
        assert_eq!(copied, expected, "fuente mono: la copia es idéntica");
    }

    #[test]
    fn analysis_thread_delivers_bands_over_channel() {
        let (mut producer, consumer) = rtrb::RingBuffer::new(RING_CAPACITY);
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let handle = analysis::spawn(consumer, Link::new(48_000), tx, stop.clone());
        // Medio segundo de un tono de 1 kHz, entregado en bloques como lo haría el audio.
        let mut source = SineWave::new(1000.0);
        for _ in 0..50 {
            for _ in 0..480 {
                let _ = producer.push(source.next().unwrap());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        stop.store(true, Ordering::Relaxed);
        handle.join().unwrap();
        let bands: Vec<Bands> = rx.try_iter().collect();
        assert!(bands.len() >= 20, "llegaron {} análisis", bands.len());
        let last = bands.last().unwrap();
        assert!(last.mid > 0.7 && last.bass < 0.2 && last.treble < 0.2, "{last:?}");
    }

    /// Simula el callback del dispositivo: pide bloques de 4096 cuadros en tiempo real a la
    /// cola de rodio, con el tap y el hilo de análisis conectados. Verifica que el análisis
    /// llega fluido (sin huecos entre bloques) y que detecta los bombos a tiempo.
    /// Tarda 10 s; solo corre si `DISKTUI_TEST_MEDIA` está definida.
    #[test]
    fn realtime_pipeline_stays_in_sync() {
        let Ok(dir) = std::env::var("DISKTUI_TEST_MEDIA") else { return };
        let decoder = Decoder::try_from(File::open(Path::new(&dir).join("test.wav")).unwrap()).unwrap();
        let (producer, consumer) = rtrb::RingBuffer::new(RING_CAPACITY);
        let link = Link::new(44_100);
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let handle = analysis::spawn(consumer, link.clone(), tx, stop.clone());
        let (player, mut output) = Player::new();
        player.append(Tap::new(decoder, producer, link));

        let start = std::time::Instant::now();
        let block = Duration::from_secs_f64(4096.0 / 44_100.0);
        let mut next = start;
        let mut events: Vec<(f32, Bands)> = Vec::new();
        while start.elapsed() < Duration::from_secs(10) {
            for _ in 0..4096 * 2 {
                output.next();
            }
            next += block;
            while std::time::Instant::now() < next {
                while let Ok(b) = rx.try_recv() {
                    events.push((start.elapsed().as_secs_f32(), b));
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        stop.store(true, Ordering::Relaxed);
        handle.join().unwrap();

        assert!(events.len() > 540, "análisis recibidos: {}", events.len());
        // Bombos: 2 por segundo. En 10 s, unos 20.
        let kicks: Vec<f32> = events.iter().filter(|(_, b)| b.kick).map(|(t, _)| *t).collect();
        assert!((18..=22).contains(&kicks.len()), "bombos: {kicks:?}");
        // Cada bombo se detecta cerca de su instante real (múltiplos de 0,5 s), con el
        // retraso propio de pedir el audio por bloques.
        for t in &kicks {
            let offset = t % 0.5;
            assert!(offset < 0.20, "bombo detectado con {offset:.3} s de retraso (t={t:.2})");
        }
        // Con el acorde sonando fuerte (segundos 4,2 a 6,2; después se apaga solo) los
        // medios no deben caer a cero entre un bloque y el siguiente.
        let gaps = events.iter().filter(|(t, b)| *t > 4.2 && *t < 6.2 && b.mid < 0.05).count();
        assert!(gaps == 0, "{gaps} análisis con medios en cero");
    }

    /// Prueba con archivos reales. Solo corre si `DISKTUI_TEST_MEDIA` apunta a una carpeta
    /// con test.mp3 (con carátula), test.flac, test.m4a, test.ogg y test.wav: bombo a
    /// 120 BPM durante 24 s, acorde desde el segundo 3, hi-hat desde el 6.
    #[test]
    fn decodes_real_files_and_finds_the_beat() {
        let Ok(dir) = std::env::var("DISKTUI_TEST_MEDIA") else { return };
        for name in ["test.mp3", "test.flac", "test.m4a", "test.ogg", "test.wav"] {
            let path = Path::new(&dir).join(name);
            let (info, cover) = meta::read(&path);
            let secs = info.duration.map(|d| d.as_secs_f32()).unwrap_or(0.0);
            assert!((secs - 24.0).abs() < 0.5, "{name}: duración {secs}");

            let decoder = Decoder::try_from(File::open(&path).unwrap()).unwrap();
            let (producer, mut consumer) = rtrb::RingBuffer::new(RING_CAPACITY);
            let link = Link::new(0);
            let mut tap = Tap::new(decoder, producer, link.clone());
            let rate = link.sample_rate.load(Ordering::Relaxed);
            let channels = tap.channels().get() as usize;
            let mut analyzer = Analyzer::new(rate);
            let hop = (rate as f32 / ANALYSIS_HZ) as usize;
            let (mut kicks, mut hats, mut frames, mut mid_late) = (0, 0, 0, 0.0f32);
            let mut block = Vec::new();
            'outer: loop {
                for _ in 0..hop * channels {
                    if tap.next().is_none() {
                        break 'outer;
                    }
                }
                block.clear();
                while let Ok(s) = consumer.pop() {
                    block.push(s);
                }
                analyzer.push(&block);
                let b = analyzer.analyze(1.0 / ANALYSIS_HZ);
                frames += 1;
                kicks += b.kick as u32;
                hats += b.hat as u32;
                if frames > 60 * 10 {
                    mid_late = mid_late.max(b.mid);
                }
            }
            assert!((42..=52).contains(&kicks), "{name}: {kicks} bombos (esperaba ~48)");
            assert!(hats > 30, "{name}: {hats} platillos");
            assert!(mid_late > 0.5, "{name}: medios {mid_late}");

            if name == "test.mp3" {
                assert_eq!(info.title, "Canción de prueba");
                assert_eq!(info.artist, "Artista ñandú");
                let palette = Palette::from_image_bytes(&cover.expect("carátula embebida")).unwrap();
                let (h, s, _) = palette.petal.to_hsl();
                assert!(s > 0.5 && (h - 0.08).abs() < 0.05, "pétalos naranjos: h={h} s={s}");
                let (sh, _, _) = palette.stem.to_hsl();
                assert!((sh - 0.64).abs() < 0.07, "tallo del azul dominante: h={sh}");
            }
        }
    }
}

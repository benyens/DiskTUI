//! Postal: guarda la escena actual como imagen PNG, o unos segundos de ella como GIF.

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use anyhow::{Context, Result, anyhow};

use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::{Cover, Palette};
use crate::text;

/// Deja solo caracteres seguros para un nombre de archivo.
pub fn file_stem(artist: &str, title: &str) -> String {
    let raw = if artist.trim().is_empty() { title.to_string() } else { format!("{artist} - {title}") };
    let clean: String = raw
        .chars()
        .map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '_' })
        .collect();
    let clean = clean.trim().trim_matches('.').to_string();
    let short: String = clean.chars().take(80).collect();
    if short.is_empty() { "disktui".into() } else { short }
}

/// Ruta nueva en `~/Pictures/DiskTUI/` para esta canción, con la extensión dada.
pub fn target(artist: &str, title: &str, extension: &str) -> Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("no se conoce la carpeta personal"))?;
    let dir = PathBuf::from(home).join("Pictures").join("DiskTUI");
    std::fs::create_dir_all(&dir).with_context(|| format!("no se pudo crear {}", dir.display()))?;
    // La hora evita pisar un archivo anterior de la misma canción.
    let stamp = std::process::Command::new("date")
        .arg("+%Y%m%d-%H%M%S")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    Ok(dir.join(format!("{} {stamp}.{extension}", file_stem(artist, title))))
}

/// Guarda el lienzo en `~/Pictures/DiskTUI/` y devuelve la ruta.
pub fn save(canvas: &Canvas, artist: &str, title: &str) -> Result<PathBuf> {
    let path = target(artist, title, "png")?;
    canvas.to_image().save(&path).with_context(|| format!("no se pudo guardar {}", path.display()))?;
    Ok(path)
}

/// Duración máxima de un GIF y cuadros por segundo.
pub const GIF_SECONDS: f32 = 8.0;
const GIF_FPS: f32 = 10.0;
/// Ancho máximo del GIF en píxeles; los lienzos más anchos se reducen a la mitad.
const GIF_MAX_WIDTH: u32 = 720;

/// Lienzo para el GIF vertical, en celdas: con la franja del título queda en 9:16.
pub const VERTICAL: (usize, usize) = (54, 40);
/// Alto de la franja del título en el GIF vertical, en píxeles.
const CAPTION_HEIGHT: u32 = 128;

/// Franja de abajo del GIF vertical: carátula, título y artista.
pub struct Caption {
    cover: Option<image::RgbImage>,
    background: Rgb,
    title_color: Rgb,
    artist_color: Rgb,
}

impl Caption {
    pub fn new(cover: Option<&Cover>, pal: &Palette) -> Self {
        let background = pal.bg_bottom.scale(0.6);
        Caption {
            cover: cover.map(|c| c.to_image(96)),
            background,
            title_color: background.mix(Rgb::WHITE, 0.92),
            artist_color: background.mix(Rgb::WHITE, 0.55),
        }
    }

    /// Pega la franja bajo la imagen de la escena.
    fn compose(&self, scene: &image::RgbImage, artist: &str, title: &str) -> image::RgbImage {
        let (w, h) = (scene.width(), scene.height());
        let (r, g, b) = self.background.to_u8();
        let mut img = image::RgbImage::from_pixel(w, h + CAPTION_HEIGHT, image::Rgb([r, g, b]));
        image::imageops::replace(&mut img, scene, 0, 0);
        let mut x = 16i64;
        if let Some(cover) = &self.cover {
            image::imageops::replace(&mut img, cover, 16, (h + 16) as i64);
            x += cover.width() as i64 + 16;
        }
        let room = w.saturating_sub(x as u32 + 12);
        text::draw(&mut img, x, (h + 30) as i64, &text::fit(title, room, 2), self.title_color, 2);
        text::draw(&mut img, x, (h + 60) as i64, &text::fit(artist, room, 1), self.artist_color, 1);
        text::draw(&mut img, x, (h + 96) as i64, "DiskTUI", self.artist_color.mix(self.background, 0.4), 1);
        img
    }
}

/// Grabación en curso: junta cuadros de la escena para armar un GIF.
pub struct Recording {
    frames: Vec<image::RgbaImage>,
    clock: f32,
    artist: String,
    title: String,
    /// GIF vertical: franja con el título bajo la escena.
    caption: Option<Caption>,
}

impl Recording {
    pub fn new(artist: &str, title: &str) -> Self {
        // El primer cuadro se toma de inmediato.
        Recording { frames: Vec::new(), clock: 1.0 / GIF_FPS, artist: artist.into(), title: title.into(), caption: None }
    }

    /// GIF vertical (9:16) con carátula, título y artista abajo. La escena se dibuja en un
    /// lienzo de [`VERTICAL`] celdas.
    pub fn vertical(artist: &str, title: &str, caption: Caption) -> Self {
        Recording { caption: Some(caption), ..Recording::new(artist, title) }
    }

    pub fn is_vertical(&self) -> bool {
        self.caption.is_some()
    }

    /// Toma un cuadro si ya toca. Devuelve `true` cuando la grabación está completa.
    pub fn capture(&mut self, canvas: &Canvas, dt: f32) -> bool {
        self.clock += dt;
        if self.clock >= 1.0 / GIF_FPS {
            self.clock -= 1.0 / GIF_FPS;
            let mut full = canvas.to_image();
            if let Some(caption) = &self.caption {
                full = caption.compose(&full, &self.artist, &self.title);
            }
            // Todos los cuadros deben medir lo mismo: si la ventana cambió, se corta ahí.
            let (w, h) = if full.width() > GIF_MAX_WIDTH { (full.width() / 2, full.height() / 2) } else { full.dimensions() };
            if self.frames.first().is_some_and(|first| first.dimensions() != (w, h)) {
                return true;
            }
            let small = if (w, h) == full.dimensions() { full } else { image::imageops::resize(&full, w, h, image::imageops::FilterType::Triangle) };
            self.frames.push(image::DynamicImage::ImageRgb8(small).into_rgba8());
        }
        self.frames.len() as f32 >= GIF_SECONDS * GIF_FPS
    }

    /// Codifica y guarda en segundo plano; el resultado llega por el canal.
    pub fn finish(self) -> mpsc::Receiver<Result<PathBuf>> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let name = if self.caption.is_some() { format!("{} (vertical)", self.title) } else { self.title.clone() };
            let result = target(&self.artist, &name, "gif").and_then(|path| {
                encode_gif(self.frames, &path)?;
                Ok(path)
            });
            let _ = tx.send(result);
        });
        rx
    }
}

fn encode_gif(frames: Vec<image::RgbaImage>, path: &Path) -> Result<()> {
    use image::codecs::gif::{GifEncoder, Repeat};
    if frames.is_empty() {
        return Err(anyhow!("la grabación quedó vacía"));
    }
    let file = std::fs::File::create(path).with_context(|| format!("no se pudo crear {}", path.display()))?;
    let mut encoder = GifEncoder::new_with_speed(std::io::BufWriter::new(file), 10);
    encoder.set_repeat(Repeat::Infinite)?;
    let delay = image::Delay::from_numer_denom_ms((1000.0 / GIF_FPS) as u32, 1);
    for frame in frames {
        encoder.encode_frame(image::Frame::from_parts(frame, 0, 0, delay))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_stem("AC/DC", "T.N.T."), "AC_DC - T.N.T");
        assert_eq!(file_stem("", "Canción: ¿qué?"), "Canción_ _qué_");
        assert_eq!(file_stem("", "///"), "___");
        assert_eq!(file_stem("", ""), "disktui");
        assert!(file_stem("a", &"x".repeat(300)).chars().count() <= 80);
    }

    #[test]
    fn recording_becomes_an_animated_gif() {
        let mut canvas = Canvas::new(20, 6);
        let mut recording = Recording::new("Banda", "Tema");
        let mut done = false;
        let mut steps = 0;
        while !done {
            canvas.clear(|x, _| crate::color::Rgb(x as f32 / 20.0, steps as f32 / 500.0, 0.3));
            done = recording.capture(&canvas, 1.0 / 60.0);
            steps += 1;
            assert!(steps < 2000, "la grabación no termina");
        }
        assert_eq!(recording.frames.len(), (GIF_SECONDS * GIF_FPS) as usize);
        // Dura lo que dice: a 60 cuadros por segundo, unos 8 segundos.
        assert!((steps as f32 / 60.0 - GIF_SECONDS).abs() < 0.5, "{steps} pasos");

        let path = std::env::temp_dir().join(format!("disktui-test-{}.gif", std::process::id()));
        recording.frames.truncate(5);
        encode_gif(recording.frames, &path).expect("codificar");
        let bytes = std::fs::read(&path).expect("leer");
        let _ = std::fs::remove_file(&path);
        assert!(bytes.starts_with(b"GIF89a"));
        let decoder = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)).expect("decodificar");
        use image::AnimationDecoder;
        assert_eq!(decoder.into_frames().count(), 5);
    }

    #[test]
    fn recording_stops_if_the_window_changes_size() {
        let mut recording = Recording::new("", "x");
        assert!(!recording.capture(&Canvas::new(20, 6), 0.2));
        assert!(recording.capture(&Canvas::new(30, 6), 0.2));
        assert_eq!(recording.frames.len(), 1);
    }

    #[test]
    fn vertical_gif_is_nine_by_sixteen_with_the_caption() {
        let mut canvas = Canvas::new(VERTICAL.0, VERTICAL.1);
        canvas.clear(|_, _| Rgb(0.2, 0.3, 0.5));
        let mut recording = Recording::vertical("Banda", "Una canción", Caption::new(None, &Palette::default()));
        recording.capture(&canvas, 1.0);
        let frame = &recording.frames[0];
        assert_eq!(frame.width() * 16, frame.height() * 9, "{}x{}", frame.width(), frame.height());
        // La franja de abajo no es la escena: tiene el fondo del pie y texto.
        let caption_row: Vec<_> = (0..frame.width()).map(|x| frame.get_pixel(x, frame.height() - 90)).collect();
        assert!(caption_row.iter().any(|p| p[0] > 200), "no hay texto en la franja");
    }
}

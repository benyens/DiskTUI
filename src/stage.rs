//! El escenario: todas las escenas juntas.
//!
//! Avanzan siempre a la vez, así al cambiar de una a otra la nueva ya va al día con la
//! canción.

use crate::aurora::Aurora;
use crate::campfire::Campfire;
use crate::canvas::Canvas;
use crate::city::City;
use crate::cosmos::Cosmos;
use crate::fireworks::Fireworks;
use crate::garden::Garden;
use crate::lofi::Lofi;
use crate::mood::Mood;
use crate::ocean::Ocean;
use crate::palette::Palette;
use crate::reef::Reef;
use crate::coast::Coast;
use crate::storm::Storm;
use crate::koi::Pond;
use crate::alley::Alley;
use crate::concert::Concert;
use crate::forest::Forest;
use crate::meadow::Meadow;
use crate::sakura::Sakura;
use crate::scenery::Scenery;
use crate::scene::{Frame, Scene, SceneKind};
use crate::train::Train;

pub struct Stage {
    pub garden: Garden,
    cosmos: Cosmos,
    ocean: Ocean,
    city: City,
    campfire: Campfire,
    train: Train,
    reef: Reef,
    fireworks: Fireworks,
    sakura: Sakura,
    meadow: Meadow,
    coast: Coast,
    forest: Forest,
    storm: Storm,
    koi: Pond,
    alley: Alley,
    concert: Concert,
    lofi: Lofi,
    aurora: Aurora,
    mood: Option<Mood>,
}

impl Stage {
    pub fn new(seed: u64, garden: Garden) -> Self {
        Stage {
            garden,
            cosmos: Cosmos::new(seed),
            ocean: Ocean::new(seed),
            city: City::new(seed),
            campfire: Campfire::new(seed),
            train: Train::new(seed),
            reef: Reef::new(seed),
            fireworks: Fireworks::new(seed),
            sakura: Sakura::new(seed),
            meadow: Meadow::new(seed),
            coast: Coast::new(seed),
            forest: Forest::new(seed),
            storm: Storm::new(seed),
            koi: Pond::new(seed),
            alley: Alley::new(seed),
            concert: Concert::new(seed),
            lofi: Lofi::new(seed),
            aurora: Aurora::new(seed),
            mood: None,
        }
    }

    /// Cambia el jardín (otra cantidad de flores) conservando el ánimo fijado.
    pub fn set_garden(&mut self, garden: Garden) {
        let clock = self.garden.scenery.clock;
        self.garden = garden;
        self.garden.scenery.force_mood(self.mood);
        self.garden.scenery.clock = clock;
    }

    /// Fija el ánimo de todas las escenas, o lo devuelve al de la canción con `None`.
    pub fn force_mood(&mut self, mood: Option<Mood>) {
        self.mood = mood;
        self.cosmos.force_mood(mood);
        for scenery in self.sceneries() {
            scenery.force_mood(mood);
        }
    }

    /// Hora del reloj (0–24) para que el cielo la siga, o `None` para que lo decida la
    /// canción. El sistema solar no tiene cielo: no cambia.
    pub fn set_clock(&mut self, hour: Option<f32>) {
        for scenery in self.sceneries() {
            scenery.clock = hour;
        }
    }

    /// Carátula de lo que suena, para las escenas que la muestran (el tocadiscos lofi).
    pub fn set_cover(&mut self, cover: Option<crate::palette::Cover>) {
        self.lofi.cover = cover;
    }

    /// El cielo compartido de cada escena que lo tiene.
    fn sceneries(&mut self) -> [&mut Scenery; 17] {
        [
            &mut self.garden.scenery,
            &mut self.ocean.scenery,
            &mut self.city.scenery,
            &mut self.campfire.scenery,
            &mut self.train.scenery,
            &mut self.reef.scenery,
            &mut self.fireworks.scenery,
            &mut self.sakura.scenery,
            &mut self.lofi.scenery,
            &mut self.aurora.scenery,
            &mut self.meadow.scenery,
            &mut self.coast.scenery,
            &mut self.forest.scenery,
            &mut self.concert.scenery,
            &mut self.alley.scenery,
            &mut self.koi.scenery,
            &mut self.storm.scenery,
        ]
    }

    fn all(&mut self) -> [&mut dyn Scene; 18] {
        [
            &mut self.garden,
            &mut self.cosmos,
            &mut self.ocean,
            &mut self.city,
            &mut self.campfire,
            &mut self.train,
            &mut self.reef,
            &mut self.fireworks,
            &mut self.sakura,
            &mut self.lofi,
            &mut self.aurora,
            &mut self.meadow,
            &mut self.coast,
            &mut self.forest,
            &mut self.concert,
            &mut self.alley,
            &mut self.koi,
            &mut self.storm,
        ]
    }

    pub fn update(&mut self, frame: &Frame, w: usize, h: usize) {
        for scene in self.all() {
            scene.update(frame, w, h);
        }
    }

    pub fn new_track(&mut self) {
        for scene in self.all() {
            scene.new_track();
        }
    }

    /// Ánimo del momento, movido por la sección que suena.
    pub fn moment(&self) -> Mood {
        self.garden.scenery.moment()
    }

    /// Un click en la escena visible, en fracciones del lienzo.
    pub fn poke(&mut self, kind: SceneKind, x: f32, y: f32) {
        let scene: &mut dyn Scene = match kind {
            SceneKind::Garden => &mut self.garden,
            SceneKind::Cosmos => &mut self.cosmos,
            SceneKind::Ocean => &mut self.ocean,
            SceneKind::City => &mut self.city,
            SceneKind::Campfire => &mut self.campfire,
            SceneKind::Train => &mut self.train,
            SceneKind::Reef => &mut self.reef,
            SceneKind::Fireworks => &mut self.fireworks,
            SceneKind::Sakura => &mut self.sakura,
            SceneKind::Meadow => &mut self.meadow,
            SceneKind::Coast => &mut self.coast,
            SceneKind::Forest => &mut self.forest,
            SceneKind::Storm => &mut self.storm,
            SceneKind::Pond => &mut self.koi,
            SceneKind::Alley => &mut self.alley,
            SceneKind::Concert => &mut self.concert,
            SceneKind::Lofi => &mut self.lofi,
            SceneKind::Aurora => &mut self.aurora,
        };
        scene.poke(x, y);
    }

    pub fn draw(&self, kind: SceneKind, canvas: &mut Canvas, pal: &Palette) {
        match kind {
            SceneKind::Garden => self.garden.draw(canvas, pal),
            SceneKind::Cosmos => self.cosmos.draw(canvas, pal),
            SceneKind::Ocean => self.ocean.draw(canvas, pal),
            SceneKind::City => self.city.draw(canvas, pal),
            SceneKind::Campfire => self.campfire.draw(canvas, pal),
            SceneKind::Train => self.train.draw(canvas, pal),
            SceneKind::Reef => self.reef.draw(canvas, pal),
            SceneKind::Fireworks => self.fireworks.draw(canvas, pal),
            SceneKind::Sakura => self.sakura.draw(canvas, pal),
            SceneKind::Meadow => self.meadow.draw(canvas, pal),
            SceneKind::Coast => self.coast.draw(canvas, pal),
            SceneKind::Forest => self.forest.draw(canvas, pal),
            SceneKind::Storm => self.storm.draw(canvas, pal),
            SceneKind::Pond => self.koi.draw(canvas, pal),
            SceneKind::Alley => self.alley.draw(canvas, pal),
            SceneKind::Concert => self.concert.draw(canvas, pal),
            SceneKind::Lofi => self.lofi.draw(canvas, pal),
            SceneKind::Aurora => self.aurora.draw(canvas, pal),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::Bands;

    fn run(stage: &mut Stage, seconds: f32, progress: f32) {
        let bands = Bands::default();
        let frame = Frame { dt: 1.0 / 60.0, bands: &bands, progress: Some(progress), song_len: 120.0, playing: true };
        for _ in 0..(seconds * 60.0) as usize {
            stage.update(&frame, 120, 40);
        }
    }

    #[test]
    fn a_click_changes_every_scene() {
        for kind in SceneKind::ALL {
            let mut quiet = Stage::new(7, Garden::new(7, 3, None));
            let mut poked = Stage::new(7, Garden::new(7, 3, None));
            run(&mut quiet, 2.0, 0.5);
            run(&mut poked, 2.0, 0.5);
            poked.poke(kind, 0.85, 0.25);
            run(&mut quiet, 0.6, 0.5);
            run(&mut poked, 0.6, 0.5);
            let (mut a, mut b) = (Canvas::new(120, 40), Canvas::new(120, 40));
            quiet.draw(kind, &mut a, &Palette::default());
            poked.draw(kind, &mut b, &Palette::default());
            assert!(a.to_image() != b.to_image(), "{} no respondió al click", kind.name());
        }
    }
}

/// Vista previa de las transiciones: `DISKTUI_PREVIEW=/carpeta cargo test preview -- --ignored`.
#[cfg(test)]
mod preview {
    use super::*;
    use crate::analysis::Bands;
    use crate::transition::{Style, Transition};

    #[test]
    #[ignore]
    fn preview_transitions() {
        let Ok(dir) = std::env::var("DISKTUI_PREVIEW") else { return };
        let mut stage = Stage::new(5, Garden::new(5, 3, None));
        let bands = Bands { level: 0.6, mid: 0.5, ..Bands::default() };
        let frame = Frame { dt: 1.0 / 60.0, bands: &bands, progress: Some(0.6), song_len: 120.0, playing: true };
        for _ in 0..600 {
            stage.update(&frame, 120, 36);
        }
        let pal = Palette::default();
        // Cada transición va de una escena a la siguiente de esta lista.
        let scenes = [SceneKind::Aurora, SceneKind::Sakura, SceneKind::Ocean, SceneKind::Meadow, SceneKind::Lofi, SceneKind::Fireworks, SceneKind::Garden];
        for (n, style) in Style::ALL.into_iter().enumerate() {
            for k in 0..=24 {
                let t = k as f32 / 24.0;
                let (mut before, mut after) = (Canvas::new(120, 36), Canvas::new(120, 36));
                stage.draw(scenes[n], &mut before, &pal);
                stage.draw(scenes[n + 1], &mut after, &pal);
                let mut tr = Transition::new(style, 11);
                tr.t = t;
                tr.apply(&mut after, &before, &pal);
                after.to_image().save(format!("{dir}/{n}_{k:02}.png")).unwrap();
            }
        }
    }
}

/// Costo por escena: `cargo test --release bench_scenes -- --ignored --nocapture`.
#[cfg(test)]
mod bench {
    use super::*;
    use crate::analysis::Bands;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    #[ignore]
    fn bench_scenes() {
        let (w, h) = (220usize, 60usize);
        let bands = Bands { level: 0.7, bass: 0.6, mid: 0.5, treble: 0.5, intensity: 0.7, ..Bands::default() };
        let pal = Palette::default();
        for kind in SceneKind::ALL {
            let mut stage = Stage::new(3, Garden::new(3, 3, None));
            let mut canvas = Canvas::new(w, h);
            let mut buf = Buffer::empty(Rect::new(0, 0, w as u16, h as u16));
            let mut frame_bands = bands;
            let (mut t_update, mut t_draw, mut t_render) = (0.0f64, 0.0f64, 0.0f64);
            let n = 240;
            for i in 0..n + 60 {
                frame_bands.beat = i % 30 == 0;
                frame_bands.kick = i % 30 == 0;
                let frame = Frame { dt: 1.0 / 60.0, bands: &frame_bands, progress: Some(0.3 + i as f32 / 2000.0), song_len: 120.0, playing: true };
                let a = std::time::Instant::now();
                stage.update(&frame, w, h);
                let b = std::time::Instant::now();
                stage.draw(kind, &mut canvas, &pal);
                let c = std::time::Instant::now();
                canvas.render(&mut buf, Rect::new(0, 0, w as u16, h as u16), crate::color::ColorMode::True);
                let d = std::time::Instant::now();
                if i >= 60 {
                    t_update += (b - a).as_secs_f64();
                    t_draw += (c - b).as_secs_f64();
                    t_render += (d - c).as_secs_f64();
                }
            }
            let ms = |t: f64| t / n as f64 * 1000.0;
            eprintln!("{:<22} update {:6.2} ms  draw {:6.2} ms  render {:6.2} ms", kind.name(), ms(t_update), ms(t_draw), ms(t_render));
        }
    }
}

/// Bytes que se mandan a la terminal por cuadro, por escena:
/// `cargo test --release bench_terminal -- --ignored --nocapture`.
#[cfg(test)]
mod bench_terminal {
    use super::*;
    use crate::analysis::Bands;
    use ratatui::backend::CrosstermBackend;
    use ratatui::layout::Rect;
    use ratatui::{Terminal, TerminalOptions, Viewport};
    use std::cell::Cell;
    use std::rc::Rc;

    /// Cuenta lo que se escribe, sin guardarlo.
    struct Count(Rc<Cell<usize>>);

    impl std::io::Write for Count {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.set(self.0.get() + buf.len());
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    #[ignore]
    fn bench_terminal() {
        let (w, h) = (200u16, 55u16);
        let pal = Palette::default();
        let mut total_all = 0usize;
        for (kind, steady_on) in SceneKind::ALL.into_iter().flat_map(|k| [(k, false), (k, true)]) {
            let mut steady = crate::steady::Steady::default();
            let mut stage = Stage::new(3, Garden::new(3, 3, None));
            let count = Rc::new(Cell::new(0));
            let backend = CrosstermBackend::new(Count(count.clone()));
            let mut term = Terminal::with_options(backend, TerminalOptions { viewport: Viewport::Fixed(Rect::new(0, 0, w, h)) }).unwrap();
            let mut canvas = Canvas::new(w as usize, h as usize);
            let mut bands = Bands { level: 0.6, bass: 0.5, mid: 0.5, treble: 0.4, intensity: 0.6, ..Bands::default() };
            let (mut bytes, n) = (0usize, 120);
            for i in 0..n + 60 {
                bands.beat = i % 30 == 0;
                bands.kick = i % 30 == 0;
                let frame = Frame { dt: 1.0 / 60.0, bands: &bands, progress: Some(0.5), song_len: 120.0, playing: true };
                stage.update(&frame, w as usize, h as usize);
                stage.draw(kind, &mut canvas, &pal);
                let before = count.get();
                term.draw(|f| {
                    let area = f.area();
                    canvas.render(f.buffer_mut(), area, crate::color::ColorMode::True);
                    if steady_on {
                        steady.apply(f.buffer_mut(), area);
                    }
                })
                .unwrap();
                if i >= 60 {
                    bytes += count.get() - before;
                }
            }
            if steady_on {
                total_all += bytes / n;
            }
            let tag = if steady_on { "estable" } else { "antes" };
            eprintln!("{:<22} {:<8} {:7} bytes/cuadro  ({:.1} MB/s a 60 FPS)", kind.name(), tag, bytes / n, bytes as f64 / n as f64 * 60.0 / 1e6);
        }
        eprintln!("promedio {} bytes/cuadro", total_all / SceneKind::ALL.len());
    }
}

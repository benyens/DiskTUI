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
use crate::meadow::Meadow;
use crate::sakura::Sakura;
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
            lofi: Lofi::new(seed),
            aurora: Aurora::new(seed),
            mood: None,
        }
    }

    /// Cambia el jardín (otra cantidad de flores) conservando el ánimo fijado.
    pub fn set_garden(&mut self, garden: Garden) {
        self.garden = garden;
        self.garden.scenery.force_mood(self.mood);
    }

    /// Fija el ánimo de todas las escenas, o lo devuelve al de la canción con `None`.
    pub fn force_mood(&mut self, mood: Option<Mood>) {
        self.mood = mood;
        self.garden.scenery.force_mood(mood);
        self.cosmos.force_mood(mood);
        self.ocean.scenery.force_mood(mood);
        self.city.scenery.force_mood(mood);
        self.campfire.scenery.force_mood(mood);
        self.train.scenery.force_mood(mood);
        self.reef.scenery.force_mood(mood);
        self.fireworks.scenery.force_mood(mood);
        self.sakura.scenery.force_mood(mood);
        self.lofi.scenery.force_mood(mood);
        self.aurora.scenery.force_mood(mood);
    }

    fn all(&mut self) -> [&mut dyn Scene; 12] {
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

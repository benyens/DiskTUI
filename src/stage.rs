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

    fn all(&mut self) -> [&mut dyn Scene; 11] {
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
            SceneKind::Lofi => self.lofi.draw(canvas, pal),
            SceneKind::Aurora => self.aurora.draw(canvas, pal),
        }
    }
}

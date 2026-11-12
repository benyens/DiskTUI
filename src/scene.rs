//! Interfaz común de las escenas: el jardín, el sistema solar, el océano, la ciudad y la fogata.

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::mood::Mood;
use crate::palette::Palette;

/// Datos de un cuadro, iguales para todas las escenas.
pub struct Frame<'a> {
    pub dt: f32,
    pub bands: &'a Bands,
    /// Avance de la canción en 0..1, si se conoce.
    pub progress: Option<f32>,
    /// Duración en segundos (o una estimación).
    pub song_len: f32,
    pub playing: bool,
}

pub trait Scene {
    /// Avanza la simulación. `w` y `h`: tamaño del lienzo en celdas.
    fn update(&mut self, frame: &Frame, w: usize, h: usize);
    fn draw(&self, canvas: &mut Canvas, pal: &Palette);
    /// Cambió la canción: la escena se deshace y vuelve a empezar.
    fn new_track(&mut self);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneKind {
    Garden,
    Cosmos,
    Ocean,
    City,
    Campfire,
}

impl SceneKind {
    #[cfg(test)]
    const ALL: [SceneKind; 5] = [SceneKind::Garden, SceneKind::Cosmos, SceneKind::Ocean, SceneKind::City, SceneKind::Campfire];

    pub fn parse(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "jardin" | "jardín" | "flores" | "garden" => Some(SceneKind::Garden),
            "sistema" | "solar" | "sistema-solar" | "cosmos" | "espacio" => Some(SceneKind::Cosmos),
            "ciudad" | "city" | "edificios" => Some(SceneKind::City),
            "fogata" | "fuego" | "campamento" | "campfire" => Some(SceneKind::Campfire),
            "oceano" | "océano" | "mar" | "velero" | "ocean" => Some(SceneKind::Ocean),
            _ => None,
        }
    }

    /// Nombre para mostrar.
    pub fn name(self) -> &'static str {
        match self {
            SceneKind::Garden => "jardín",
            SceneKind::Cosmos => "sistema solar",
            SceneKind::Ocean => "océano",
            SceneKind::City => "ciudad",
            SceneKind::Campfire => "fogata",
        }
    }

    /// Nombre para la configuración y la línea de comandos.
    pub fn key(self) -> &'static str {
        match self {
            SceneKind::Garden => "jardin",
            SceneKind::Cosmos => "sistema",
            SceneKind::Ocean => "oceano",
            SceneKind::City => "ciudad",
            SceneKind::Campfire => "fogata",
        }
    }

    /// La escena que mejor le va a cada ánimo, para el modo automático.
    pub fn for_mood(mood: Mood) -> Self {
        match mood {
            Mood::Happy => SceneKind::Garden,
            Mood::Serene => SceneKind::Campfire,
            Mood::Sad => SceneKind::Ocean,
            Mood::Nostalgic => SceneKind::Cosmos,
            Mood::Intense => SceneKind::City,
        }
    }

    pub fn next(self) -> Self {
        match self {
            SceneKind::Garden => SceneKind::Cosmos,
            SceneKind::Cosmos => SceneKind::Ocean,
            SceneKind::Ocean => SceneKind::City,
            SceneKind::City => SceneKind::Campfire,
            SceneKind::Campfire => SceneKind::Garden,
        }
    }
}

/// Lo que elige el usuario: una escena fija, o que la elija la canción según su ánimo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneChoice {
    Auto,
    Fixed(SceneKind),
}

impl SceneChoice {
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "auto" | "automatica" | "automática" => Some(SceneChoice::Auto),
            other => SceneKind::parse(other).map(SceneChoice::Fixed),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            SceneChoice::Auto => "automática",
            SceneChoice::Fixed(kind) => kind.name(),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            SceneChoice::Auto => "auto",
            SceneChoice::Fixed(kind) => kind.key(),
        }
    }

    /// Recorre: automática, jardín, sistema solar, océano, ciudad, fogata, y de vuelta.
    pub fn next(self) -> Self {
        match self {
            SceneChoice::Auto => SceneChoice::Fixed(SceneKind::Garden),
            SceneChoice::Fixed(SceneKind::Campfire) => SceneChoice::Auto,
            SceneChoice::Fixed(kind) => SceneChoice::Fixed(kind.next()),
        }
    }

    /// La escena a mostrar con este ánimo.
    pub fn resolve(self, mood: Mood) -> SceneKind {
        match self {
            SceneChoice::Auto => SceneKind::for_mood(mood),
            SceneChoice::Fixed(kind) => kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choice_cycles_through_every_scene_and_back_to_auto() {
        let mut choice = SceneChoice::Auto;
        let mut seen = Vec::new();
        for _ in 0..6 {
            seen.push(choice);
            choice = choice.next();
        }
        assert_eq!(choice, SceneChoice::Auto);
        assert_eq!(seen.len(), 6);
        for kind in SceneKind::ALL {
            assert!(seen.contains(&SceneChoice::Fixed(kind)));
            assert_eq!(SceneChoice::parse(kind.key()), Some(SceneChoice::Fixed(kind)));
        }
        assert_eq!(SceneChoice::parse("auto"), Some(SceneChoice::Auto));
        assert_eq!(SceneChoice::parse("nada"), None);
    }

    #[test]
    fn every_scene_is_reachable_in_auto_mode() {
        let scenes: Vec<SceneKind> = Mood::ALL.into_iter().map(SceneKind::for_mood).collect();
        for kind in SceneKind::ALL {
            assert!(scenes.contains(&kind), "{kind:?} no sale nunca");
        }
        assert_eq!(SceneChoice::Fixed(SceneKind::City).resolve(Mood::Happy), SceneKind::City);
    }
}

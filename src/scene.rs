//! Interfaz común de las escenas: el jardín, el sistema solar, el océano y la ciudad.

use crate::analysis::Bands;
use crate::canvas::Canvas;
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
}

impl SceneKind {
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "jardin" | "jardín" | "flores" | "garden" => Some(SceneKind::Garden),
            "sistema" | "solar" | "sistema-solar" | "cosmos" | "espacio" => Some(SceneKind::Cosmos),
            "ciudad" | "city" | "edificios" => Some(SceneKind::City),
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
        }
    }

    /// Nombre para la configuración y la línea de comandos.
    pub fn key(self) -> &'static str {
        match self {
            SceneKind::Garden => "jardin",
            SceneKind::Cosmos => "sistema",
            SceneKind::Ocean => "oceano",
            SceneKind::City => "ciudad",
        }
    }

    pub fn next(self) -> Self {
        match self {
            SceneKind::Garden => SceneKind::Cosmos,
            SceneKind::Cosmos => SceneKind::Ocean,
            SceneKind::Ocean => SceneKind::City,
            SceneKind::City => SceneKind::Garden,
        }
    }
}

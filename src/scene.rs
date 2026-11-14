//! Interfaz común de las escenas: el jardín, el sistema solar, el océano, la ciudad, la fogata, el tren y el fondo del mar.

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SceneKind {
    Garden,
    Cosmos,
    Ocean,
    City,
    Campfire,
    Train,
    Reef,
}

impl SceneKind {
    #[cfg(test)]
    const ALL: [SceneKind; 7] =
        [SceneKind::Garden, SceneKind::Cosmos, SceneKind::Ocean, SceneKind::City, SceneKind::Campfire, SceneKind::Train, SceneKind::Reef];

    pub fn parse(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "jardin" | "jardín" | "flores" | "garden" => Some(SceneKind::Garden),
            "sistema" | "solar" | "sistema-solar" | "cosmos" | "espacio" => Some(SceneKind::Cosmos),
            "ciudad" | "city" | "edificios" => Some(SceneKind::City),
            "fogata" | "fuego" | "campamento" | "campfire" => Some(SceneKind::Campfire),
            "tren" | "train" | "tren nocturno" => Some(SceneKind::Train),
            "fondo" | "fondo del mar" | "submarino" | "acuario" | "reef" => Some(SceneKind::Reef),
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
            SceneKind::Train => "tren nocturno",
            SceneKind::Reef => "fondo del mar",
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
            SceneKind::Train => "tren",
            SceneKind::Reef => "fondo",
        }
    }

    /// Las escenas que le van a cada ánimo, para el modo automático. La primera es la de
    /// siempre; las demás se reparten entre canciones.
    pub fn for_mood_all(mood: Mood) -> &'static [SceneKind] {
        match mood {
            Mood::Happy => &[SceneKind::Garden],
            Mood::Serene => &[SceneKind::Campfire, SceneKind::Reef],
            Mood::Sad => &[SceneKind::Ocean, SceneKind::Train],
            Mood::Nostalgic => &[SceneKind::Cosmos, SceneKind::Train],
            Mood::Intense => &[SceneKind::City],
        }
    }

    /// La escena principal de cada ánimo.
    pub fn for_mood(mood: Mood) -> Self {
        Self::for_mood_all(mood)[0]
    }

    /// La escena para esta canción: entre las que le van a su ánimo, una fija por canción
    /// (`salt` sale del título y el artista), para que no se repita siempre la misma.
    pub fn for_song(mood: Mood, salt: u64) -> Self {
        let options = Self::for_mood_all(mood);
        options[(salt % options.len() as u64) as usize]
    }

    /// Número fijo para una canción, para [`SceneKind::for_song`].
    pub fn salt(artist: &str, title: &str) -> u64 {
        // FNV-1a: estable entre versiones, a diferencia del hash de la biblioteca estándar.
        artist.bytes().chain([0]).chain(title.bytes()).fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
    }

    pub fn next(self) -> Self {
        match self {
            SceneKind::Garden => SceneKind::Cosmos,
            SceneKind::Cosmos => SceneKind::Ocean,
            SceneKind::Ocean => SceneKind::City,
            SceneKind::City => SceneKind::Campfire,
            SceneKind::Campfire => SceneKind::Train,
            SceneKind::Train => SceneKind::Reef,
            SceneKind::Reef => SceneKind::Garden,
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

    /// Recorre: automática y todas las escenas, y de vuelta.
    pub fn next(self) -> Self {
        match self {
            SceneChoice::Auto => SceneChoice::Fixed(SceneKind::Garden),
            SceneChoice::Fixed(SceneKind::Reef) => SceneChoice::Auto,
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
        for _ in 0..8 {
            seen.push(choice);
            choice = choice.next();
        }
        assert_eq!(choice, SceneChoice::Auto);
        assert_eq!(seen.len(), 8);
        for kind in SceneKind::ALL {
            assert!(seen.contains(&SceneChoice::Fixed(kind)));
            assert_eq!(SceneChoice::parse(kind.key()), Some(SceneChoice::Fixed(kind)));
        }
        assert_eq!(SceneChoice::parse("auto"), Some(SceneChoice::Auto));
        assert_eq!(SceneChoice::parse("nada"), None);
    }

    #[test]
    fn every_scene_is_reachable_in_auto_mode() {
        let scenes: Vec<SceneKind> = Mood::ALL.into_iter().flat_map(|m| SceneKind::for_mood_all(m).iter().copied()).collect();
        for kind in SceneKind::ALL {
            assert!(scenes.contains(&kind), "{kind:?} no sale nunca");
        }
        assert_eq!(SceneChoice::Fixed(SceneKind::City).resolve(Mood::Happy), SceneKind::City);
    }

    #[test]
    fn each_song_keeps_its_scene_and_songs_spread_over_the_options() {
        let a = SceneKind::salt("Banda", "Tema uno");
        assert_eq!(a, SceneKind::salt("Banda", "Tema uno"));
        assert_ne!(a, SceneKind::salt("Banda", "Tema dos"));
        let picks: std::collections::HashSet<SceneKind> =
            (0..20).map(|i| SceneKind::for_song(Mood::Serene, SceneKind::salt("x", &i.to_string()))).collect();
        assert_eq!(picks.len(), 2, "{picks:?}");
        for _ in 0..3 {
            assert_eq!(SceneKind::for_song(Mood::Happy, a), SceneKind::Garden);
        }
    }
}

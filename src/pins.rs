//! Escenas fijadas por canción: con `E`, la canción que suena queda con la escena que se
//! ve, y en automática siempre sale con esa (aunque no esté en la lista).
//!
//! Se guardan en `escenas.tsv`, una canción por línea: artista, título y escena.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::scene::SceneKind;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Pins {
    songs: BTreeMap<(String, String), SceneKind>,
}

/// Las tabulaciones y saltos de línea separan campos y canciones: en los textos se cambian.
fn clean(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ").trim().to_string()
}

fn path() -> Option<PathBuf> {
    Some(crate::paths::data_dir()?.join("escenas.tsv"))
}

impl Pins {
    pub fn parse(text: &str) -> Self {
        let songs = text
            .lines()
            .filter_map(|line| {
                let mut parts = line.split('\t');
                let (artist, title, scene) = (parts.next()?, parts.next()?, parts.next()?);
                let scene = SceneKind::parse(scene.trim())?;
                let title = clean(title);
                (!title.is_empty()).then(|| ((clean(artist), title), scene))
            })
            .collect();
        Pins { songs }
    }

    pub fn to_text(&self) -> String {
        self.songs.iter().map(|((artist, title), scene)| format!("{artist}\t{title}\t{}\n", scene.key())).collect()
    }

    /// Si no se puede leer (primera vez, sin permisos), empieza vacío.
    pub fn load() -> Self {
        path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| Self::parse(&t)).unwrap_or_default()
    }

    pub fn get(&self, artist: &str, title: &str) -> Option<SceneKind> {
        self.songs.get(&(clean(artist), clean(title))).copied()
    }

    /// Fija la escena de la canción, o la suelta si ya tenía esa. Devuelve cómo quedó.
    pub fn toggle(&mut self, artist: &str, title: &str, scene: SceneKind) -> Option<SceneKind> {
        let song = (clean(artist), clean(title));
        if song.1.is_empty() {
            return None;
        }
        if self.songs.get(&song) == Some(&scene) {
            self.songs.remove(&song);
            None
        } else {
            self.songs.insert(song, scene);
            Some(scene)
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(file) = path() else { return Ok(()) };
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(file, self.to_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_and_unpins() {
        let mut pins = Pins::default();
        assert_eq!(pins.toggle("Banda", "Tema", SceneKind::Coast), Some(SceneKind::Coast));
        assert_eq!(pins.get(" Banda", "Tema "), Some(SceneKind::Coast));
        // Otra escena la cambia; la misma la suelta.
        assert_eq!(pins.toggle("Banda", "Tema", SceneKind::Storm), Some(SceneKind::Storm));
        assert_eq!(pins.toggle("Banda", "Tema", SceneKind::Storm), None);
        assert_eq!(pins.get("Banda", "Tema"), None);
        // Sin título no se fija.
        assert_eq!(pins.toggle("Banda", "  ", SceneKind::Storm), None);
    }

    #[test]
    fn survives_a_round_trip_and_skips_junk() {
        let mut pins = Pins::default();
        pins.toggle("A\tB", "Tema\nuno", SceneKind::Village);
        pins.toggle("Otra", "Canción", SceneKind::Pond);
        let back = Pins::parse(&pins.to_text());
        assert_eq!(back, pins);
        assert_eq!(back.get("A B", "Tema uno"), Some(SceneKind::Village));
        let junk = Pins::parse("solo\nA\tB\tnoexiste\n\t\tjardin\nX\tY\tjardin\n");
        assert_eq!(junk.songs.len(), 1);
        assert_eq!(junk.get("X", "Y"), Some(SceneKind::Garden));
    }
}

//! Canciones favoritas: se marcan con `f` mientras suenan y el herbario puede mostrar solo esas.
//!
//! Se guardan aparte del herbario (`favoritas.tsv`, una canción por línea: artista y título),
//! así una canción se puede marcar aunque todavía no haya dejado su flor.

use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Favorites {
    songs: BTreeSet<(String, String)>,
}

/// Las tabulaciones y saltos de línea separan campos y canciones: en los textos se cambian.
fn clean(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ").trim().to_string()
}

fn path() -> Option<PathBuf> {
    Some(crate::paths::data_dir()?.join("favoritas.tsv"))
}

impl Favorites {
    pub fn parse(text: &str) -> Self {
        let songs = text
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .map(|(artist, title)| (clean(artist), clean(title)))
            .filter(|(_, title)| !title.is_empty())
            .collect();
        Favorites { songs }
    }

    pub fn to_text(&self) -> String {
        self.songs.iter().map(|(artist, title)| format!("{artist}\t{title}\n")).collect()
    }

    /// Si no se puede leer (primera vez, sin permisos), empieza vacía.
    pub fn load() -> Self {
        path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| Self::parse(&t)).unwrap_or_default()
    }

    pub fn contains(&self, artist: &str, title: &str) -> bool {
        self.songs.contains(&(clean(artist), clean(title)))
    }

    /// Marca o desmarca la canción. Devuelve si quedó como favorita.
    pub fn toggle(&mut self, artist: &str, title: &str) -> bool {
        let song = (clean(artist), clean(title));
        if song.1.is_empty() {
            return false;
        }
        if self.songs.remove(&song) {
            false
        } else {
            self.songs.insert(song);
            true
        }
    }

    /// Guarda la lista. Si falla, la marca vale igual hasta cerrar el programa.
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
    fn toggle_marks_and_unmarks() {
        let mut fav = Favorites::default();
        assert!(fav.toggle("OVERSIZE", "Dissolve"));
        assert!(fav.contains("OVERSIZE", "Dissolve"));
        assert!(!fav.contains("OVERSIZE", "Otra"));
        assert!(!fav.toggle("OVERSIZE", "Dissolve"));
        assert!(!fav.contains("OVERSIZE", "Dissolve"));
    }

    #[test]
    fn roundtrips_through_the_file_format() {
        let mut fav = Favorites::default();
        fav.toggle("Artista\tcon tab", "Canción");
        fav.toggle("", "Sin artista");
        let back = Favorites::parse(&fav.to_text());
        assert_eq!(back, fav);
        assert_eq!(back.songs.len(), 2);
        assert!(back.contains("Artista con tab", "Canción"));
    }

    #[test]
    fn songs_without_title_are_ignored() {
        let mut fav = Favorites::default();
        assert!(!fav.toggle("Alguien", "  "));
        assert_eq!(Favorites::parse("solo una columna\nA\t\n").songs.len(), 0);
    }
}

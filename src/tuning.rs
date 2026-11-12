//! Correcciones de ánimo: cuando el análisis se equivoca con una canción, el usuario la
//! corrige desde el panel de análisis y DiskTUI lo recuerda.
//!
//! Se guardan en `~/.local/share/disktui/animos.tsv`, una canción por línea, junto con lo
//! que el análisis había medido. Esa pareja (medición, ánimo correcto) sirve después para
//! calibrar la heurística.

use std::path::PathBuf;

use crate::features::Song;
use crate::mood::Mood;
use crate::session::TrackInfo;

struct Row {
    artist: String,
    title: String,
    mood: Mood,
    /// Lo medido al corregir, tal como va al archivo.
    measured: String,
}

#[derive(Default)]
pub struct Corrections {
    rows: Vec<Row>,
    path: Option<PathBuf>,
}

fn clean(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
}

fn measured(song: &Song) -> String {
    format!(
        "{:.1}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}",
        song.bpm, song.majorness, song.brightness, song.density, song.valence, song.arousal
    )
}

impl Corrections {
    /// Lee las correcciones guardadas. Sin archivo, empieza vacío.
    pub fn load() -> Self {
        let path = std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".local").join("share").join("disktui").join("animos.tsv"));
        let text = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        Corrections { rows: Self::parse(&text), path }
    }

    fn parse(text: &str) -> Vec<Row> {
        text.lines()
            .filter_map(|line| {
                let mut parts = line.splitn(4, '\t');
                let (artist, title, mood) = (parts.next()?, parts.next()?, parts.next()?);
                Some(Row {
                    artist: artist.into(),
                    title: title.into(),
                    mood: Mood::parse(mood)?,
                    measured: parts.next().unwrap_or("").into(),
                })
            })
            .collect()
    }

    fn text(&self) -> String {
        self.rows
            .iter()
            .map(|r| format!("{}\t{}\t{}\t{}\n", r.artist, r.title, r.mood.name(), r.measured))
            .collect()
    }

    /// Ánimo corregido para esta canción, si lo hay.
    pub fn get(&self, track: &TrackInfo) -> Option<Mood> {
        let (artist, title) = (clean(&track.artist), clean(&track.title));
        self.rows.iter().find(|r| r.artist == artist && r.title == title).map(|r| r.mood)
    }

    /// Fija (o borra, con `None`) el ánimo de la canción. Solo las canciones reales se
    /// guardan en disco; las demás valen hasta salir.
    pub fn set(&mut self, track: &TrackInfo, mood: Option<Mood>, song: &Song) {
        let (artist, title) = (clean(&track.artist), clean(&track.title));
        self.rows.retain(|r| !(r.artist == artist && r.title == title));
        if let Some(mood) = mood {
            self.rows.push(Row { artist, title, mood, measured: measured(song) });
        }
        if track.known && let Some(path) = &self.path {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, self.text());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(title: &str) -> TrackInfo {
        TrackInfo { title: title.into(), artist: "Una\tBanda".into(), duration: None, known: false }
    }

    #[test]
    fn corrections_are_remembered_per_song_and_survive_the_file_format() {
        let mut c = Corrections::default();
        let song = Song { bpm: 120.0, valence: 0.7, arousal: 0.6, ..Song::default() };
        assert_eq!(c.get(&track("Uno")), None);
        c.set(&track("Uno"), Some(Mood::Sad), &song);
        c.set(&track("Dos"), Some(Mood::Happy), &song);
        c.set(&track("Uno"), Some(Mood::Intense), &song);
        assert_eq!(c.get(&track("Uno")), Some(Mood::Intense));
        assert_eq!(c.get(&track("Dos")), Some(Mood::Happy));

        let again = Corrections { rows: Corrections::parse(&c.text()), path: None };
        assert_eq!(again.get(&track("Uno")), Some(Mood::Intense));
        assert_eq!(again.rows.len(), 2);
        assert!(again.rows[0].measured.starts_with("120.0\t"));

        c.set(&track("Uno"), None, &song);
        assert_eq!(c.get(&track("Uno")), None);
    }
}

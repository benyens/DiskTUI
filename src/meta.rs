//! Etiquetas y carátula embebida del archivo de audio (vía `lofty`).

use std::path::Path;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::PictureType;
use lofty::tag::Accessor;

use crate::session::TrackInfo;

/// Lee título, artista, duración y los bytes de la carátula. Nunca falla: si el archivo no
/// tiene etiquetas, usa el nombre del archivo como título.
pub fn read(path: &Path) -> (TrackInfo, Option<Vec<u8>>) {
    let fallback = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "sin título".into());
    let mut info = TrackInfo { title: fallback, artist: String::new(), duration: None, known: true };

    let Ok(tagged) = lofty::read_from_path(path) else {
        return (info, None);
    };
    let duration = tagged.properties().duration();
    if !duration.is_zero() {
        info.duration = Some(duration);
    }

    let mut cover = None;
    // Etiqueta principal primero; luego las demás, por si la carátula está en otra.
    let tags = tagged.primary_tag().into_iter().chain(tagged.tags());
    let mut have_title = false;
    for tag in tags {
        let title = tag.title().filter(|t| !t.trim().is_empty());
        if let (false, Some(title)) = (have_title, title) {
            info.title = title.into_owned();
            have_title = true;
        }
        let artist = tag.artist().filter(|a| !a.trim().is_empty());
        if let (true, Some(artist)) = (info.artist.is_empty(), artist) {
            info.artist = artist.into_owned();
        }
        if cover.is_none() {
            let pictures = tag.pictures();
            let front = pictures.iter().find(|p| p.pic_type() == PictureType::CoverFront);
            cover = front.or(pictures.first()).map(|p| p.data().to_vec());
        }
    }
    (info, cover)
}

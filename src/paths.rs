//! Dónde guarda DiskTUI sus archivos, y la fecha y hora locales.
//!
//! En macOS y Linux: `~/.config/disktui` (configuración), `~/.local/share/disktui` (herbario,
//! correcciones) y `~/Pictures/DiskTUI` (postales, GIF, pósters). En Windows, la
//! configuración y los datos van a `%APPDATA%\DiskTUI` y las imágenes a la carpeta
//! Imágenes del usuario.

use std::path::PathBuf;

/// Carpeta personal: `HOME`, o `USERPROFILE` en Windows.
pub fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

/// Carpeta de la configuración.
pub fn config_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        return appdata();
    }
    home().map(|h| h.join(".config").join("disktui"))
}

/// Carpeta de los datos: herbario y correcciones de ánimo.
pub fn data_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        return appdata();
    }
    home().map(|h| h.join(".local").join("share").join("disktui"))
}

fn appdata() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(|a| PathBuf::from(a).join("DiskTUI"))
        .or_else(|| home().map(|h| h.join("DiskTUI")))
}

/// Carpeta de las imágenes que guarda el programa.
pub fn pictures_dir() -> Option<PathBuf> {
    home().map(|h| h.join("Pictures").join("DiskTUI"))
}

/// Cómo nombrar la carpeta de imágenes en los avisos.
pub fn pictures_label() -> &'static str {
    if cfg!(windows) { "Imágenes\\DiskTUI" } else { "~/Pictures/DiskTUI" }
}

/// Fecha local de hoy, `AAAA-MM-DD`.
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Fecha y hora local para nombres de archivo, `AAAAMMDD-HHMMSS`.
pub fn stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_have_the_expected_shape() {
        let today = today();
        assert_eq!(today.len(), 10, "{today}");
        assert_eq!(&today[4..5], "-");
        let stamp = stamp();
        assert_eq!(stamp.len(), 15, "{stamp}");
        assert_eq!(&stamp[8..9], "-");
        assert!(stamp.starts_with(&today.replace('-', "")));
    }

    #[test]
    fn folders_hang_from_the_home_folder() {
        if let Some(home) = home() {
            assert!(pictures_dir().unwrap().starts_with(&home));
            if !cfg!(windows) {
                assert!(config_dir().unwrap().ends_with(".config/disktui"));
                assert!(data_dir().unwrap().ends_with(".local/share/disktui"));
            }
        }
    }
}

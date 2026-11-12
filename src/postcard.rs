//! Postal: guarda la escena actual como imagen PNG.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};

use crate::canvas::Canvas;

/// Deja solo caracteres seguros para un nombre de archivo.
pub fn file_stem(artist: &str, title: &str) -> String {
    let raw = if artist.trim().is_empty() { title.to_string() } else { format!("{artist} - {title}") };
    let clean: String = raw
        .chars()
        .map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '_' })
        .collect();
    let clean = clean.trim().trim_matches('.').to_string();
    let short: String = clean.chars().take(80).collect();
    if short.is_empty() { "disktui".into() } else { short }
}

/// Guarda el lienzo en `~/Pictures/DiskTUI/` y devuelve la ruta.
pub fn save(canvas: &Canvas, artist: &str, title: &str) -> Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("no se conoce la carpeta personal"))?;
    let dir = PathBuf::from(home).join("Pictures").join("DiskTUI");
    std::fs::create_dir_all(&dir).with_context(|| format!("no se pudo crear {}", dir.display()))?;
    // La hora evita pisar una postal anterior de la misma canción.
    let stamp = std::process::Command::new("date")
        .arg("+%Y%m%d-%H%M%S")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let path = dir.join(format!("{} {stamp}.png", file_stem(artist, title)));
    canvas.to_image().save(&path).with_context(|| format!("no se pudo guardar {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_stem("AC/DC", "T.N.T."), "AC_DC - T.N.T");
        assert_eq!(file_stem("", "Canción: ¿qué?"), "Canción_ _qué_");
        assert_eq!(file_stem("", "///"), "___");
        assert_eq!(file_stem("", ""), "disktui");
        assert!(file_stem("a", &"x".repeat(300)).chars().count() <= 80);
    }
}

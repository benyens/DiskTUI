//! Letras sincronizadas: se muestran abajo, línea por línea, siguiendo la posición de la
//! canción en Spotify (o la app que suene).
//!
//! Vienen de LRCLIB (lrclib.net), una base de letras libre que no pide cuenta. Es opcional
//! (tecla `l`, fila *Letras* del menú o `--letras`): es la única consulta a internet aparte
//! de la carátula, y solo manda artista, título y duración. Cada letra se guarda en
//! `letras/` junto al herbario, así no se vuelve a pedir; ahí también se puede dejar un
//! `.lrc` propio con el nombre `Artista - Título.lrc`.

use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, SystemTime};

/// Una línea de la letra y el segundo en que empieza.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub at: f32,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lyrics {
    pub lines: Vec<Line>,
}

/// Lee una marca `[mm:ss.xx]` (también `[mm:ss]` o `[mm:ss:xx]`). `None` si es otra cosa,
/// como `[ar: artista]`.
fn timestamp(tag: &str) -> Option<f32> {
    let (min, rest) = tag.split_once(':')?;
    let min: f32 = min.trim().parse().ok()?;
    let rest = rest.trim().replacen(':', ".", 1);
    let sec: f32 = rest.parse().ok()?;
    (min >= 0.0 && (0.0..60.0).contains(&sec)).then_some(min * 60.0 + sec)
}

impl Lyrics {
    /// Interpreta un archivo LRC. Una línea puede tener varias marcas (un coro que se repite).
    pub fn parse(text: &str) -> Lyrics {
        let mut lines = Vec::new();
        for raw in text.lines() {
            let mut rest = raw.trim();
            let mut times = Vec::new();
            while let Some(after) = rest.strip_prefix('[') {
                let Some(end) = after.find(']') else { break };
                match timestamp(&after[..end]) {
                    Some(t) => times.push(t),
                    None => break,
                }
                rest = after[end + 1..].trim_start();
            }
            // Las marcas de palabra (`<00:12.34>`) del formato extendido no se muestran.
            let mut text = String::new();
            let mut in_tag = false;
            for c in rest.chars() {
                match c {
                    '<' => in_tag = true,
                    '>' if in_tag => in_tag = false,
                    c if !in_tag => text.push(c),
                    _ => {}
                }
            }
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            for at in times {
                lines.push(Line { at, text: text.clone() });
            }
        }
        lines.sort_by(|a, b| a.at.total_cmp(&b.at));
        Lyrics { lines }
    }

    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|l| l.text.is_empty())
    }

    /// La línea que suena en el segundo `t`, si ya empezó alguna.
    pub fn index_at(&self, t: f32) -> Option<usize> {
        let after = self.lines.partition_point(|l| l.at <= t);
        after.checked_sub(1)
    }

    /// Avance dentro de la línea `i`, de 0 a 1 (para iluminarla de a poco). Una línea larga
    /// sin la siguiente se da por cantada en 4 s.
    pub fn line_progress(&self, i: usize, t: f32) -> f32 {
        let Some(line) = self.lines.get(i) else { return 0.0 };
        let end = self.lines.get(i + 1).map(|n| n.at).unwrap_or(line.at + 4.0);
        let span = (end - line.at).clamp(0.5, 6.0);
        ((t - line.at) / span).clamp(0.0, 1.0)
    }
}

/// Qué canción pedir.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub artist: String,
    pub title: String,
    pub duration: Option<f32>,
}

impl Request {
    /// Nombre del archivo en caché: `Artista - Título`, sin caracteres raros.
    fn file_stem(&self) -> String {
        let name = format!("{} - {}", self.artist.trim(), self.title.trim());
        name.chars().map(|c| if c.is_alphanumeric() || " -_.,'()&!".contains(c) { c } else { '_' }).take(120).collect()
    }
}

fn cache_dir() -> Option<PathBuf> {
    Some(crate::paths::data_dir()?.join("letras"))
}

/// Si la búsqueda falló hace menos de esto, no se vuelve a preguntar: LRCLIB va sumando
/// letras con el tiempo.
const RETRY_AFTER: Duration = Duration::from_secs(3 * 24 * 3600);

/// Lo que hay guardado: `Some(Some(letra))`, `Some(None)` si hace poco no se encontró,
/// `None` si nunca se buscó.
fn from_cache(req: &Request) -> Option<Option<Lyrics>> {
    let dir = cache_dir()?;
    let stem = req.file_stem();
    if let Ok(text) = std::fs::read_to_string(dir.join(format!("{stem}.lrc"))) {
        let lyrics = Lyrics::parse(&text);
        return Some((!lyrics.is_empty()).then_some(lyrics));
    }
    let missing = std::fs::metadata(dir.join(format!("{stem}.sin-letra"))).ok()?;
    let age = SystemTime::now().duration_since(missing.modified().ok()?).unwrap_or_default();
    (age < RETRY_AFTER).then_some(None)
}

fn to_cache(req: &Request, synced: Option<&str>) {
    let Some(dir) = cache_dir() else { return };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let stem = req.file_stem();
    let _ = match synced {
        Some(text) => std::fs::write(dir.join(format!("{stem}.lrc")), text),
        None => std::fs::write(dir.join(format!("{stem}.sin-letra")), ""),
    };
}

/// Pide a LRCLIB. Devuelve el JSON, o `None` si no hubo respuesta.
fn lrclib(endpoint: &str, params: &[(&str, String)]) -> Option<serde_json::Value> {
    let mut cmd = Command::new("curl");
    cmd.args(["-sG", "--max-time", "8", "--proto", "=https"]);
    cmd.args(["-H", concat!("User-Agent: DiskTUI v", env!("CARGO_PKG_VERSION"), " (https://github.com/benyens/DiskTUI)")]);
    for (key, value) in params {
        cmd.arg("--data-urlencode").arg(format!("{key}={value}"));
    }
    cmd.arg(format!("https://lrclib.net/api/{endpoint}"));
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice(&out.stdout).ok()
}

/// La letra sincronizada de un registro de LRCLIB, si la tiene.
fn synced_of(record: &serde_json::Value) -> Option<String> {
    let text = record.get("syncedLyrics")?.as_str()?;
    (!text.trim().is_empty()).then(|| text.to_string())
}

/// Entre los resultados de una búsqueda, el que tiene letra sincronizada y la duración
/// más parecida (a lo más 5 s de diferencia si se conoce).
pub fn best_match(results: &serde_json::Value, duration: Option<f32>) -> Option<String> {
    let mut best: Option<(f32, String)> = None;
    for record in results.as_array()? {
        let Some(text) = synced_of(record) else { continue };
        let gap = match (duration, record.get("duration").and_then(|d| d.as_f64())) {
            (Some(want), Some(have)) => (want - have as f32).abs(),
            (Some(_), None) => 10.0,
            (None, _) => 0.0,
        };
        if duration.is_some() && gap > 5.0 {
            continue;
        }
        if best.as_ref().is_none_or(|(g, _)| gap < *g) {
            best = Some((gap, text));
        }
    }
    best.map(|(_, text)| text)
}

/// Busca la letra: primero exacta (artista, título y duración), después por búsqueda.
/// `Err` si no hubo conexión: entonces no se guarda que falta.
fn fetch(req: &Request) -> Result<Option<String>, ()> {
    let mut params = vec![("track_name", req.title.clone()), ("artist_name", req.artist.clone())];
    if let Some(d) = req.duration.filter(|d| (1.0..3600.0).contains(d)) {
        params.push(("duration", format!("{}", d.round() as u32)));
    }
    let exact = lrclib("get", &params).ok_or(())?;
    if let Some(text) = synced_of(&exact) {
        return Ok(Some(text));
    }
    let found = lrclib("search", &params[..2]).ok_or(())?;
    Ok(best_match(&found, req.duration))
}

/// Busca letras en segundo plano para no trabar la animación.
pub struct Fetcher {
    requests: Sender<Request>,
    results: Receiver<(Request, Option<Lyrics>)>,
}

impl Fetcher {
    pub fn new() -> Self {
        let (requests, request_rx) = mpsc::channel::<Request>();
        let (result_tx, results) = mpsc::channel();
        let _ = thread::Builder::new().name("disktui-letras".into()).spawn(move || {
            while let Ok(mut req) = request_rx.recv() {
                // Si cambiaron de canción varias veces seguidas, solo importa la última.
                while let Ok(newer) = request_rx.try_recv() {
                    req = newer;
                }
                let lyrics = match from_cache(&req) {
                    Some(cached) => cached,
                    None => match fetch(&req) {
                        Ok(text) => {
                            to_cache(&req, text.as_deref());
                            text.map(|t| Lyrics::parse(&t)).filter(|l| !l.is_empty())
                        }
                        Err(()) => None,
                    },
                };
                if result_tx.send((req, lyrics)).is_err() {
                    break;
                }
            }
        });
        Fetcher { requests, results }
    }

    pub fn request(&self, req: Request) {
        let _ = self.requests.send(req);
    }

    /// La respuesta más reciente que haya llegado.
    pub fn poll(&self) -> Option<(Request, Option<Lyrics>)> {
        let mut last = None;
        while let Ok(r) = self.results.try_recv() {
            last = Some(r);
        }
        last
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lrc_with_metadata_repeats_and_word_marks() {
        let text = "[ar: Alguien]\n[ti: Canción]\n[00:12.50]Primera línea\n[00:20.00][01:05.25]Coro <00:20.50>que se <00:21.00>repite\n[00:30]\n[1:10:50]Fin";
        let l = Lyrics::parse(text);
        let times: Vec<f32> = l.lines.iter().map(|x| x.at).collect();
        assert_eq!(times, vec![12.5, 20.0, 30.0, 65.25, 70.5]);
        assert_eq!(l.lines[1].text, "Coro que se repite");
        assert_eq!(l.lines[3].text, "Coro que se repite");
        assert_eq!(l.lines[2].text, "");
        assert!(!l.is_empty());
        assert!(Lyrics::parse("[ar: nadie]\nsolo texto").is_empty());
    }

    #[test]
    fn finds_the_line_that_is_playing() {
        let l = Lyrics::parse("[00:10.00]a\n[00:15.00]b\n[00:30.00]c");
        assert_eq!(l.index_at(5.0), None);
        assert_eq!(l.index_at(10.0), Some(0));
        assert_eq!(l.index_at(14.9), Some(0));
        assert_eq!(l.index_at(31.0), Some(2));
        assert!((l.line_progress(0, 12.5) - 0.5).abs() < 1e-4);
        assert_eq!(l.line_progress(2, 100.0), 1.0);
    }

    #[test]
    fn picks_the_synced_result_with_the_closest_duration() {
        let results = serde_json::json!([
            {"duration": 200.0, "syncedLyrics": null, "plainLyrics": "x"},
            {"duration": 230.0, "syncedLyrics": "[00:01.00]lejos"},
            {"duration": 203.0, "syncedLyrics": "[00:01.00]cerca"},
            {"duration": 201.0, "syncedLyrics": "  "},
        ]);
        assert_eq!(best_match(&results, Some(201.0)).as_deref(), Some("[00:01.00]cerca"));
        assert_eq!(best_match(&results, Some(400.0)), None);
        assert!(best_match(&results, None).is_some());
        assert_eq!(best_match(&serde_json::json!({"statusCode": 404}), Some(1.0)), None);
    }

    #[test]
    fn cache_names_are_safe() {
        let req = Request { artist: "AC/DC".into(), title: "T.N.T: \"live\"?".into(), duration: None };
        let stem = req.file_stem();
        assert!(!stem.contains('/') && !stem.contains('"') && !stem.contains('?') && !stem.contains(':'));
        assert!(stem.starts_with("AC_DC - T.N.T"));
    }
}

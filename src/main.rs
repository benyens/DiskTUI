//! DiskTUI: visualizador de audio en la terminal. Una planta crece desde la tierra con los
//! graves y florece con los medios y agudos, con los colores de la carátula.

mod analysis;
mod audio;
mod aurora;
mod campfire;
mod canvas;
mod city;
mod capture;
mod color;
mod cosmos;
mod demo;
mod diagnose;
mod favorites;
mod features;
mod fireworks;
mod garden;
mod herbarium;
mod idle;
mod lofi;
mod meadow;
mod menu;
#[cfg(windows)]
mod media_windows;
mod meta;
mod moment;
mod mood;
mod ocean;
mod palette;
mod paths;
mod plant;
mod poster;
mod postcard;
mod reef;
mod sakura;
mod render;
mod scene;
mod scenery;
mod session;
mod spotify;
mod stage;
mod structure;
mod text;
mod train;
mod tuning;
#[cfg(all(target_os = "macos", feature = "tap-app"))]
mod tap_macos;
mod ui;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event, KeyCode,
    KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::DefaultTerminal;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use canvas::Canvas;
use color::ColorMode;
use menu::{MenuResult, Settings, Source};
use mood::Mood;
use palette::Palette;
use garden::Garden;
use scene::{Frame, SceneChoice, SceneKind};
use stage::Stage;
use session::Session;

/// Segundos que saltan las flechas.
const SEEK_STEP: f32 = 10.0;

const HELP: &str = "\
DiskTUI: tu música, creciendo en la terminal.

USO
  disktui                   Abre el menú de inicio (por defecto: Spotify)
  disktui <archivo>         Reproduce un archivo (mp3, flac, wav, ogg, m4a)
  disktui --spotify         Visualiza lo que suena en la app de Spotify
  disktui --apple-music     Lo mismo con Apple Music (experimental)
  disktui --demo            Animación con señal sintética, sin audio
  disktui --herbario        Muestra las flores de las canciones que ya escuchaste
  disktui --diagnostico     Prueba el modo Spotify paso a paso y dice qué falla

OPCIONES
  --escena <nombre>         auto (según el ánimo), jardin, sistema, oceano, ciudad,
                            fogata, tren, fondo, fuegos, cerezos, lofi, aurora o mariposas
  --fps <n>                 Cuadros por segundo (por defecto 60)
  --petalos <n>             Pétalos de la flor principal, 3 a 12 (por defecto según la canción)
  --flores <n>              Cantidad de flores, 1 a 3 (por defecto 3)
  --velocidad <x>           Qué tan rápido crece la escena; 2 = el doble (por defecto 1)
  --animo <nombre>          Fija el clima: alegre, serena, nostalgica, triste o intensa
                            (por defecto lo decide la canción)
  --caratula                Muestra la carátula del disco en una esquina
  --mini                    Para un panel chico (tmux): HUD de una línea, una flor, 30 FPS
  --sin-tarjeta             No muestra el título grande al empezar cada canción
  --sin-hud                 Oculta el HUD (título, barra de reproducción)
  --dispositivo <nombre>    Modo Spotify: captura desde esa entrada (ej. \"BlackHole\")
  --global                  Modo Spotify: captura todo el audio del sistema
  -h, --help                Esta ayuda

TECLAS
  q / Esc    salir          espacio   pausa
  + / -      volumen        h         mostrar u ocultar el HUD
  e          cambiar de escena (automática y las doce escenas)
  g          grabar un GIF de 8 segundos en ~/Pictures/DiskTUI
  v          grabar un GIF vertical (9:16) con carátula y título, para historias
  d          panel de detalles: qué entendió de la canción, la fuente de audio,
             los avisos completos y cómo corregir el ánimo
  c          mostrar u ocultar la carátula
  t          encender o apagar la tarjeta con el título de cada canción
  click      cada escena responde: un fuego artificial, una mariposa, un pez, un cometa…
  ← / →      retroceder / adelantar 10 segundos (archivo, Spotify, Apple Music)
  n / p      canción siguiente / anterior (Spotify y Apple Music)
  s          guardar una postal PNG en ~/Pictures/DiskTUI
  f          marcar o desmarcar la canción como favorita (★ en el herbario)
  m          volver al menú
";

enum Mode {
    File(PathBuf),
    Spotify,
    Music,
    Demo,
    Diagnose,
    Herbarium,
}

struct Args {
    /// `None` = sin fuente en la línea de comandos: se abre el menú.
    mode: Option<Mode>,
    fps: u32,
    petals: Option<u32>,
    flowers: Option<u32>,
    scene: Option<SceneChoice>,
    speed: Option<f32>,
    mood: Option<Mood>,
    hud: bool,
    /// Mostrar la carátula en una esquina.
    cover: bool,
    /// Tarjeta con el título al empezar cada canción.
    card: bool,
    /// Modo mini, para un panel chico: HUD de una línea, una flor, 30 FPS.
    mini: bool,
    device: Option<String>,
    global: bool,
    /// Se pidió Apple Music en vez de Spotify.
    music: bool,
    /// Depuración: `--dump SEG[,ANCHOxALTO]` imprime un cuadro del demo y sale.
    dump: Option<(f32, u16, u16)>,
    /// Depuración: semilla del sistema solar en `--dump`.
    seed: Option<u64>,
}

fn parse_args() -> Result<Args> {
    let mut args = Args {
        mode: None,
        fps: 60,
        petals: None,
        flowers: None,
        scene: None,
        speed: None,
        mood: None,
        hud: true,
        cover: false,
        card: true,
        mini: false,
        device: None,
        global: false,
        music: false,
        dump: None,
        seed: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| anyhow::anyhow!("falta el valor de {name}"));
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                std::process::exit(0);
            }
            "--spotify" => args.mode = Some(Mode::Spotify),
            "--apple-music" | "--music" => {
                args.music = true;
                // Si ya se pidió el diagnóstico, se mantiene: solo cambia la app.
                if !matches!(args.mode, Some(Mode::Diagnose)) {
                    args.mode = Some(Mode::Music);
                }
            }
            "--demo" => args.mode = Some(Mode::Demo),
            "--diagnostico" | "--diagnóstico" => args.mode = Some(Mode::Diagnose),
            "--herbario" => args.mode = Some(Mode::Herbarium),
            "--sin-hud" => args.hud = false,
            "--mini" => args.mini = true,
            "--caratula" | "--carátula" => args.cover = true,
            "--sin-tarjeta" => args.card = false,
            "--global" => args.global = true,
            "--fps" => args.fps = value("--fps")?.parse::<u32>()?.clamp(10, 240),
            "--petalos" | "--pétalos" => args.petals = Some(value("--petalos")?.parse::<u32>()?.clamp(3, 12)),
            "--escena" => {
                let name = value("--escena")?;
                args.scene = Some(
                    SceneChoice::parse(&name)
                        .ok_or_else(|| anyhow::anyhow!("escena desconocida: {name} (usa auto, jardin, sistema, oceano, ciudad, fogata, tren, fondo, fuegos, cerezos, lofi, aurora o mariposas)"))?,
                );
            }
            "--velocidad" => {
                args.speed = Some(value("--velocidad")?.replace(',', ".").parse::<f32>()?.clamp(0.25, 8.0));
            }
            "--animo" | "--ánimo" => {
                let name = value("--animo")?;
                args.mood = Some(Mood::parse(&name).ok_or_else(|| {
                    anyhow::anyhow!("ánimo desconocido: {name} (usa alegre, serena, nostalgica, triste o intensa)")
                })?);
            }
            "--flores" => args.flowers = Some(value("--flores")?.parse::<u32>()?.clamp(1, 3)),
            "--dispositivo" => args.device = Some(value("--dispositivo")?),
            "--semilla" => args.seed = Some(value("--semilla")?.parse()?),
            "--dump" => {
                let spec = value("--dump")?;
                let (secs, size) = spec.split_once(',').unwrap_or((&spec, "100x36"));
                let (w, h) = size.split_once('x').unwrap_or(("100", "36"));
                args.dump = Some((secs.parse()?, w.parse()?, h.parse()?));
            }
            other if other.starts_with('-') => bail!("opción desconocida: {other}\n\n{HELP}"),
            path => args.mode = Some(Mode::File(PathBuf::from(path))),
        }
    }
    Ok(args)
}

fn main() {
    // Los errores se imprimen en una línea, con su causa, y sin traza.
    if let Err(e) = start() {
        eprintln!("Error: {e:#}");
        std::process::exit(1);
    }
}

/// Cómo terminó una sesión de visualización.
#[derive(PartialEq, Eq)]
enum RunEnd {
    Quit,
    /// El usuario pidió el menú, o terminó la canción habiendo venido del menú.
    Menu,
}

/// Opciones de una sesión de visualización.
struct RunOptions {
    fps: u32,
    petals: Option<u32>,
    flowers: u32,
    scene: SceneChoice,
    speed: f32,
    /// Clima fijo; `None` = el que pida la canción.
    mood: Option<Mood>,
    hud: bool,
    cover: bool,
    card: bool,
    mini: bool,
    /// Al terminar la canción se vuelve al menú en vez de salir.
    from_menu: bool,
}

type Opened = (Box<dyn Session>, u64);

fn open_file(path: &std::path::Path) -> Result<Opened> {
    if !path.is_file() {
        bail!("no existe el archivo: {}", path.display());
    }
    let seed = hash_str(&path.to_string_lossy());
    Ok((Box::new(audio::FileSession::open(path)?), seed))
}

fn open_player(app: spotify::App, device: Option<String>, global: bool) -> Result<Opened> {
    Ok((Box::new(spotify::SpotifySession::open(app, device, global)?), 0x5107))
}

fn open_demo() -> Opened {
    (Box::new(demo::DemoSession::new(75.0)), 0xD3E0)
}

fn start() -> Result<()> {
    let args = parse_args()?;
    if let Some((secs, w, h)) = args.dump {
        print!("{}", dump_frame(secs, w, h, &args));
        return Ok(());
    }

    // Lo guardado en el menú es el punto de partida; la línea de comandos manda sobre eso.
    let mut settings = Settings::load();
    if let Some(scene) = args.scene {
        settings.scene = scene;
    }
    if let Some(flowers) = args.flowers {
        settings.flowers = flowers;
    }
    if let Some(speed) = args.speed {
        settings.speed = speed;
    }
    if args.mood.is_some() {
        settings.mood = args.mood;
    }
    settings.global |= args.global;
    settings.cover |= args.cover;
    settings.card &= args.card;

    // Con fuente en la línea de comandos se abre antes de tomar la terminal: así los
    // errores se leen bien. Sin fuente, se abre el menú.
    let first: Option<Opened> = match &args.mode {
        None => None,
        Some(Mode::Diagnose) => {
            // `--diagnostico --apple-music` diagnostica Apple Music; solo, Spotify.
            let app = if args.music { spotify::App::Music } else { spotify::App::Spotify };
            if cfg!(windows) {
                anyhow::bail!("el diagnóstico es del modo Spotify, que todavía no funciona en Windows");
            }
            diagnose::run(app, args.device.clone());
            return Ok(());
        }
        Some(Mode::File(path)) => Some(open_file(path)?),
        Some(Mode::Spotify) => {
            // Abrir la captura prueba cada método un momento: puede tardar unos segundos.
            eprintln!("Buscando el audio de Spotify…");
            Some(open_player(spotify::App::Spotify, args.device.clone(), settings.global)?)
        }
        Some(Mode::Music) => {
            eprintln!("Buscando el audio de Apple Music…");
            Some(open_player(spotify::App::Music, args.device.clone(), settings.global)?)
        }
        Some(Mode::Demo) => Some(open_demo()),
        Some(Mode::Herbarium) => None,
    };

    let mut terminal = ratatui::init();
    // Captura del mouse: así el scroll llega como evento de mouse (que se ignora) y la
    // terminal deja de traducirlo a flechas. Pegado entre corchetes: arrastrar un archivo
    // al menú llega como un solo evento.
    // Por separado: si una falla (el pegado entre corchetes en consolas antiguas de
    // Windows), la otra se aplica igual.
    let _ = crossterm::execute!(std::io::stdout(), EnableMouseCapture);
    let _ = crossterm::execute!(std::io::stdout(), EnableBracketedPaste);
    // Si el programa entra en pánico, también hay que soltar el mouse antes de salir.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::execute!(std::io::stdout(), DisableMouseCapture, DisableBracketedPaste);
        previous(info);
    }));
    let result = if matches!(args.mode, Some(Mode::Herbarium)) {
        herbarium::view(&mut terminal, ColorMode::detect())
    } else {
        launch(&mut terminal, &args, &mut settings, first)
    };
    let _ = crossterm::execute!(std::io::stdout(), DisableMouseCapture, DisableBracketedPaste);
    ratatui::restore();
    result
}

/// Ciclo menú → visualización → menú. Con `first`, la primera vuelta se salta el menú.
fn launch(
    terminal: &mut DefaultTerminal,
    args: &Args,
    settings: &mut Settings,
    mut first: Option<Opened>,
) -> Result<()> {
    let mode = ColorMode::detect();
    let mut error: Option<String> = None;
    loop {
        let from_menu = first.is_none();
        let opened = match first.take() {
            Some(opened) => opened,
            None => {
                match menu::run(terminal, settings, mode, error.take())? {
                    MenuResult::Quit => return Ok(()),
                    MenuResult::Herbarium => {
                        herbarium::view(terminal, mode)?;
                        continue;
                    }
                    MenuResult::Start => {}
                }
                settings.save();
                let attempt = match settings.source {
                    Source::Demo => Ok(open_demo()),
                    Source::File if settings.file.trim().is_empty() => {
                        Err(anyhow::anyhow!("falta la ruta del archivo"))
                    }
                    Source::File => open_file(std::path::Path::new(&settings.file)),
                    Source::Spotify => {
                        menu::draw_notice(terminal, "Buscando el audio de Spotify…", mode)?;
                        open_player(spotify::App::Spotify, args.device.clone(), settings.global)
                    }
                    Source::Music => {
                        menu::draw_notice(terminal, "Buscando el audio de Apple Music…", mode)?;
                        open_player(spotify::App::Music, args.device.clone(), settings.global)
                    }
                };
                match attempt {
                    Ok(opened) => opened,
                    // El error se muestra dentro del menú, sin salir del programa.
                    Err(e) => {
                        error = Some(format!("{e:#}"));
                        continue;
                    }
                }
            }
        };
        let options = RunOptions {
            fps: if args.mini { args.fps.min(30) } else { args.fps },
            petals: args.petals,
            flowers: if args.mini && args.flowers.is_none() { 1 } else { settings.flowers },
            scene: settings.scene,
            speed: settings.speed,
            mood: settings.mood,
            hud: args.hud,
            cover: settings.cover,
            card: settings.card,
            mini: args.mini,
            from_menu,
        };
        let (session, seed) = opened;
        if run(terminal, session, seed, &options)? == RunEnd::Quit {
            return Ok(());
        }
    }
}

fn hash_str(s: &str) -> u64 {
    // FNV-1a: la misma canción da siempre la misma planta.
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

fn run(
    terminal: &mut DefaultTerminal,
    mut session: Box<dyn Session>,
    seed: u64,
    args: &RunOptions,
) -> Result<RunEnd> {
    let mode = ColorMode::detect();
    let frame_time = Duration::from_secs_f64(1.0 / args.fps as f64);
    let mut stage = Stage::new(seed, Garden::new(seed, args.flowers, args.petals));
    stage.force_mood(args.mood);
    let mut cover: Option<palette::Cover> = None;
    let mut show_cover = args.cover;
    // Tarjeta de canción: segundos desde que empezó la canción actual.
    let mut show_card = args.card;
    let mut card_clock: Option<f32> = Some(0.0);
    // Escena: fija, o la que pida el ánimo de la canción. En automático se elige una vez
    // por canción, cuando el análisis ya es fiable.
    let mut choice = args.scene;
    let mut active = choice.resolve(args.mood.unwrap_or(Mood::Happy));
    let mut chosen_for_track = false;
    // Cambio de escena: la anterior sigue viva y se funde con la nueva (escena, avance,
    // duración en segundos).
    let mut scene_fade: Option<(SceneKind, f32, f32)> = None;
    let mut before = Canvas::new(0, 0);
    // Cambio de canción: el último cuadro de la anterior se disuelve en la nueva.
    let mut song_fade: Option<(Canvas, f32)> = None;
    let mut tracks_seen = 0u32;
    let mut drop_moment = moment::DropMoment::default();
    let mut portrait = Canvas::new(0, 0);
    let mut idle = idle::Idle::default();
    let mut idle_canvas = Canvas::new(0, 0);
    // Panel de análisis y correcciones de ánimo guardadas por canción.
    let mut show_analysis = false;
    // Grabación de GIF en curso, y el guardado en segundo plano de la anterior.
    let mut recording: Option<postcard::Recording> = None;
    let mut gif_job: Option<std::sync::mpsc::Receiver<Result<std::path::PathBuf>>> = None;
    let mut corrections = tuning::Corrections::load();
    let mut favorites = favorites::Favorites::load();
    let mut applied_mood = args.mood;
    let mut palette = Palette::default();
    let mut target_palette = palette;
    let mut canvas = Canvas::new(1, 1);
    let mut show_hud = args.hud;
    let mut timeline = ui::Timeline::new();
    // Para el herbario: qué canción suena, cuánto de ella se escuchó y qué se entendió.
    let mut current = session.track().clone();
    let mut listened = 0.0f32;
    let mut song = features::Song::default();
    let mut notice: Option<(String, Instant)> = None;
    let mut last = Instant::now();
    let mut next_frame = Instant::now();
    // Al terminar el archivo, la planta queda unos segundos antes de salir.
    let mut outro: Option<f32> = None;

    loop {
        // Teclado: se atiende sin bloquear.
        while event::poll(Duration::ZERO)? {
            let event = event::read()?;
            // Un click en la escena: cada una responde a su manera.
            if let Event::Mouse(mouse) = &event {
                if mouse.kind == MouseEventKind::Down(MouseButton::Left) && (mouse.row as usize) < canvas.h && canvas.w > 0 {
                    let x = (mouse.column as f32 + 0.5) / canvas.w as f32;
                    let y = (mouse.row as f32 + 0.5) / canvas.h as f32;
                    stage.poke(active, x, y);
                }
                continue;
            }
            if let Event::Key(key) = event {
                // En Windows llegan también los eventos de soltar la tecla: se ignoran, o
                // cada tecla contaría dos veces.
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        finish_gif(recording.take(), gif_job.take());
                        return Ok(RunEnd::Quit);
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(RunEnd::Quit);
                    }
                    KeyCode::Char('m') => {
                        finish_gif(recording.take(), gif_job.take());
                        return Ok(RunEnd::Menu);
                    }
                    KeyCode::Char(' ') => session.toggle_pause(),
                    // Con la captura del mouse activa, el scroll ya no llega como flechas.
                    KeyCode::Right => session.seek(SEEK_STEP),
                    KeyCode::Left => session.seek(-SEEK_STEP),
                    // Volumen en + y −, no en las flechas: en pantalla completa la terminal
                    // convierte el scroll del trackpad en flechas y el volumen se volvía loco.
                    KeyCode::Char('+') | KeyCode::Char('=') => session.change_volume(1),
                    KeyCode::Char('-') | KeyCode::Char('_') => session.change_volume(-1),
                    KeyCode::Char('h') => show_hud = !show_hud,
                    KeyCode::Char('c') => show_cover = !show_cover,
                    KeyCode::Char('t') => {
                        show_card = !show_card;
                        let text = if show_card { "tarjeta de canción: encendida" } else { "tarjeta de canción: apagada" };
                        notice = Some((text.to_string(), Instant::now()));
                    }
                    KeyCode::Char('e') => {
                        choice = choice.next();
                        chosen_for_track = false;
                        if let SceneChoice::Fixed(kind) = choice
                            && kind != active
                        {
                            scene_fade = Some((active, 0.0, 0.6));
                            active = kind;
                        }
                        notice = Some((format!("escena: {}", choice.name()), Instant::now()));
                    }
                    KeyCode::Char('g') => {
                        let text = if let Some(rec) = recording.take() {
                            gif_job = Some(rec.finish());
                            "guardando el GIF…".to_string()
                        } else if gif_job.is_some() {
                            "todavía se está guardando el GIF anterior".to_string()
                        } else {
                            recording = Some(postcard::Recording::new(&current.artist, &current.title));
                            format!("● grabando GIF ({:.0} s) · g para cortar antes", postcard::GIF_SECONDS)
                        };
                        notice = Some((text, Instant::now()));
                    }
                    KeyCode::Char('v') => {
                        let text = if recording.is_some() || gif_job.is_some() {
                            "ya hay un GIF en curso".to_string()
                        } else {
                            let caption = postcard::Caption::new(cover.as_ref(), &palette);
                            recording = Some(postcard::Recording::vertical(&current.artist, &current.title, caption));
                            format!("● grabando GIF vertical ({:.0} s) · g para cortar antes", postcard::GIF_SECONDS)
                        };
                        notice = Some((text, Instant::now()));
                    }
                    KeyCode::Char('d') => show_analysis = !show_analysis,
                    // Con el panel abierto, los números corrigen el ánimo de esta canción.
                    KeyCode::Char(digit @ '0'..='5') if show_analysis => {
                        let mood = Mood::ALL.get((digit as usize).wrapping_sub('1' as usize)).copied();
                        corrections.set(&current, mood, &song);
                        chosen_for_track = false;
                        let text = match mood {
                            Some(m) => format!("ánimo de esta canción: {}", m.name()),
                            None => "ánimo de esta canción: automático".to_string(),
                        };
                        notice = Some((text, Instant::now()));
                    }
                    KeyCode::Char('f') => {
                        let text = if current.title.trim().is_empty() {
                            "esta canción no tiene título: no se puede marcar".to_string()
                        } else {
                            let on = favorites.toggle(&current.artist, &current.title);
                            match (on, favorites.save()) {
                                (_, Err(e)) => format!("no se pudo guardar la favorita: {e}"),
                                (true, Ok(())) => "★ marcada como favorita".to_string(),
                                (false, Ok(())) => "ya no es favorita".to_string(),
                            }
                        };
                        notice = Some((text, Instant::now()));
                    }
                    KeyCode::Char('n') => session.skip(1),
                    KeyCode::Char('p') => session.skip(-1),
                    KeyCode::Char('s') => {
                        let text = match postcard::save(&canvas, &current.artist, &current.title) {
                            Ok(_) => format!("postal guardada en {}", paths::pictures_label()),
                            Err(e) => format!("no se pudo guardar la postal: {e:#}"),
                        };
                        notice = Some((text, Instant::now()));
                    }
                    _ => {}
                }
            }
        }

        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f32().clamp(0.0005, 0.1);
        last = now;

        // Datos de audio: llegan del hilo de análisis por el canal.
        let tick = session.tick(dt);
        if let Some(p) = tick.palette {
            target_palette = p;
            cover = tick.cover;
        }
        if tick.new_track {
            if collect(&current, listened, &song, &stage.garden, &target_palette) {
                notice = Some((format!("✿ \"{}\" quedó en tu herbario", current.title), Instant::now()));
            }
            listened = 0.0;
            tracks_seen += 1;
            // El último cuadro de la canción anterior se funde con la nueva.
            if tracks_seen > 1 {
                song_fade = Some((canvas.clone(), 0.0));
            }
            stage.new_track();
            timeline.reset();
            chosen_for_track = false;
        }
        if session.playing() {
            listened += dt;
        }
        song = tick.bands.song;
        if session.track().title != current.title || session.track().artist != current.artist {
            current = session.track().clone();
            card_clock = Some(0.0);
            chosen_for_track = false;
        }
        palette = palette.mix(&target_palette, 1.0 - (-dt * 2.5).exp());

        // Ánimo en uso: el de `--animo`, si no la corrección guardada, si no el del análisis.
        let wanted_mood = args.mood.or_else(|| corrections.get(&current));
        if wanted_mood != applied_mood {
            applied_mood = wanted_mood;
            stage.force_mood(wanted_mood);
        }
        let heard_mood = Mood::nearest(song.valence, song.arousal);
        let mood_now = wanted_mood.unwrap_or(heard_mood);
        if choice == SceneChoice::Auto && !chosen_for_track && (wanted_mood.is_some() || song.settled >= 0.95) {
            chosen_for_track = true;
            let target = SceneKind::for_song(mood_now, SceneKind::salt(&current.artist, &current.title));
            if target != active {
                // En la primera canción no hay nada que fundir: se cambia de inmediato.
                let first = tracks_seen <= 1 && session.position().as_secs_f32() < 1.5;
                if !first {
                    scene_fade = Some((active, 0.0, 2.2));
                }
                active = target;
            }
        }
        if let Some((_, t, length)) = &mut scene_fade {
            *t += dt / *length;
            if *t >= 1.0 {
                scene_fade = None;
            }
        }
        if let Some((_, t)) = &mut song_fade {
            *t += dt / 2.5;
            if *t >= 1.0 {
                song_fade = None;
            }
        }
        // El drop: se sabe cuándo viene el próximo clímax si la canción está pre-analizada.
        let position = session.position().as_secs_f32();
        let next_climax = session.song_map().and_then(|m| m.climaxes.iter().copied().find(|&c| c > position)).map(|c| c - position);
        drop_moment.update(dt, next_climax, tick.bands.climax, session.playing());

        if session.finished() {
            let t = outro.get_or_insert(0.0);
            *t += dt;
            // Si hay un GIF grabándose o guardándose, se espera a que termine.
            if *t > 4.0 && recording.is_none() && gif_job.is_none() {
                collect(&current, listened, &song, &stage.garden, &target_palette);
                return Ok(if args.from_menu { RunEnd::Menu } else { RunEnd::Quit });
            }
        }

        let song_len = session.track().duration.map(|d| d.as_secs_f32()).unwrap_or(210.0);
        let size = terminal.size()?;
        let hud_rows = ui::rows(show_hud, size.height).min(if args.mini { 1 } else { 3 });
        canvas.resize(size.width as usize, (size.height - hud_rows) as usize);

        // Barra de reproducción: la canción entera si ya se pre-analizó; si no, lo que suena.
        if !timeline.is_complete() && let Some(map) = session.song_map() {
            timeline.load_map(map, song_len);
        }
        if session.playing() && let Some(p) = session.progress() {
            timeline.record(p, tick.bands.intensity, tick.bands.climax);
        }

        // Las dos escenas avanzan siempre: al cambiar con `e`, la otra ya va al día.
        // `--velocidad` adelanta el reloj de las escenas respecto a la canción.
        let frame = Frame {
            dt: dt * drop_moment.time_scale(),
            bands: &tick.bands,
            progress: session.progress().map(|p| (p * args.speed).min(1.0)),
            song_len: song_len / args.speed,
            playing: session.playing(),
        };
        stage.update(&frame, canvas.w, canvas.h);
        stage.draw(active, &mut canvas, &palette);
        if let Some((from, t, _)) = scene_fade {
            before.resize(canvas.w, canvas.h);
            stage.draw(from, &mut before, &palette);
            canvas.crossfade(&before, plant::smoothstep(0.0, 1.0, t));
        }
        if let Some((old, t)) = &song_fade {
            canvas.crossfade(old, plant::smoothstep(0.0, 1.0, *t));
        }
        drop_moment.apply(&mut canvas, palette.accent.mix(color::Rgb::WHITE, 0.6));
        // Sin música por un rato: la pantalla de espera.
        idle.update(dt, session.playing());
        if idle.level() > 0.0 {
            idle_canvas.resize(canvas.w, canvas.h);
            idle.draw(&mut idle_canvas, &palette);
            idle_canvas.crossfade(&canvas, plant::smoothstep(0.0, 1.0, idle.level()));
            std::mem::swap(&mut canvas, &mut idle_canvas);
        }
        if show_cover && let Some(cover) = &cover {
            cover.draw(&mut canvas, render::SOIL_ROWS * 2 + 8, palette.accent);
        }
        // El GIF vertical se dibuja aparte, en su propio lienzo de 9:16.
        let vertical = recording.as_ref().is_some_and(postcard::Recording::is_vertical);
        if vertical {
            portrait.resize(postcard::VERTICAL.0, postcard::VERTICAL.1);
            stage.draw(active, &mut portrait, &palette);
            drop_moment.apply(&mut portrait, palette.accent.mix(color::Rgb::WHITE, 0.6));
        }
        let shot = if vertical { &portrait } else { &canvas };
        if recording.as_mut().is_some_and(|rec| rec.capture(shot, dt)) {
            gif_job = recording.take().map(postcard::Recording::finish);
            notice = Some(("guardando el GIF…".to_string(), Instant::now()));
        }
        // Tarjeta de canción (después de grabar: los GIF quedan limpios).
        card_clock = card_clock.map(|t| t + dt).filter(|t| *t < ui::CARD_SECONDS);
        let card_alpha = if show_card && !args.mini { card_clock.map(ui::card_alpha).unwrap_or(0.0) } else { 0.0 };
        if card_alpha > 0.0
            && let Some((top, bottom)) = ui::card_band(canvas.h as u16)
        {
            canvas.shade(top as usize * 2, bottom as usize * 2 + 2, 0.6 * card_alpha);
        }
        if let Some(result) = gif_job.as_ref().and_then(|job| job.try_recv().ok()) {
            gif_job = None;
            let text = match result {
                Ok(_) => format!("GIF guardado en {}", paths::pictures_label()),
                Err(e) => format!("no se pudo guardar el GIF: {e:#}"),
            };
            notice = Some((text, Instant::now()));
        }

        terminal.draw(|frame| {
            let area = frame.area();
            let plant_area = Rect { height: area.height - hud_rows.min(area.height), ..area };
            canvas.render(frame.buffer_mut(), plant_area, mode);
            if card_alpha > 0.0 {
                let card = ui::Card { title: &current.title, artist: &current.artist, alpha: card_alpha, pal: &palette, mode };
                ui::draw_card(frame.buffer_mut(), plant_area, &card);
            }
            if hud_rows > 0 {
                let hud_area = Rect { y: area.bottom() - hud_rows, height: hud_rows, ..area };
                let hud = ui::Hud {
                    session: session.as_ref(),
                    pal: &palette,
                    mode,
                    timeline: &timeline,
                    bands: &tick.bands,
                    scene: active,
                    auto_scene: choice == SceneChoice::Auto,
                    favorite: favorites.contains(&session.track().artist, &session.track().title),
                    notice: notice.as_ref().filter(|(_, at)| at.elapsed() < Duration::from_secs(5)).map(|(t, _)| t.as_str()),
                };
                ui::draw(frame.buffer_mut(), hud_area, &hud);
            }
            if show_analysis {
                let origin = if args.mood.is_some() {
                    "fijo"
                } else if wanted_mood.is_some() {
                    "corregido"
                } else {
                    "auto"
                };
                let analysis = ui::Analysis {
                    song: &song,
                    mood: mood_now,
                    origin,
                    moment: stage.moment(),
                    source: session.label(),
                    warning: session.warning().map(|w| w.detail),
                    pal: &palette,
                    mode,
                };
                ui::draw_analysis(frame.buffer_mut(), plant_area, &analysis);
            }
        })?;

        // Ritmo fijo de cuadros.
        next_frame += frame_time;
        let now = Instant::now();
        if next_frame > now {
            std::thread::sleep(next_frame - now);
        } else {
            next_frame = now;
        }
    }
}

/// Guarda la flor de la canción en el herbario si se escuchó al menos el 60%. Devuelve si
/// la guardó.
fn collect(track: &session::TrackInfo, listened: f32, song: &features::Song, garden: &Garden, pal: &Palette) -> bool {
    if !worth_collecting(track, listened) {
        return false;
    }
    let genes = garden.main().genes.unwrap_or_else(|| plant::Genes::from_song(song));
    herbarium::record(herbarium::Entry::new(&track.title, &track.artist, song, genes, pal));
    true
}

/// ¿La canción merece una flor? Debe ser real (no el demo ni un aviso), durar más de 20 s y
/// haberse escuchado al menos el 60%.
fn worth_collecting(track: &session::TrackInfo, listened: f32) -> bool {
    let Some(total) = track.duration.map(|d| d.as_secs_f32()).filter(|t| *t > 20.0) else { return false };
    track.known && listened >= 0.6 * total
}

/// Simula el demo hasta `secs` y devuelve un cuadro como texto con colores ANSI.
/// Al salir con un GIF a medias: lo grabado hasta ahí se guarda igual, y se espera a que
/// termine de escribirse.
fn finish_gif(recording: Option<postcard::Recording>, job: Option<std::sync::mpsc::Receiver<Result<std::path::PathBuf>>>) {
    for job in recording.map(postcard::Recording::finish).into_iter().chain(job) {
        let _ = job.recv();
    }
}

fn dump_frame(secs: f32, w: u16, h: u16, args: &Args) -> String {
    use ratatui::style::Color;
    let mut session = demo::DemoSession::new(75.0);
    let choice = args.scene.unwrap_or(SceneChoice::Fixed(SceneKind::Garden));
    let kind = choice.resolve(args.mood.unwrap_or(Mood::Happy));
    let speed = args.speed.unwrap_or(1.0);
    let mut stage = Stage::new(args.seed.unwrap_or(0xD3E0), Garden::new(0xD3E0, args.flowers.unwrap_or(3), args.petals));
    stage.force_mood(args.mood);
    let palette = Palette::default();
    let hud_rows = ui::rows(args.hud, h);
    let mut canvas = Canvas::new(w as usize, (h - hud_rows) as usize);
    let mut timeline = ui::Timeline::new();
    let dt = 1.0 / 60.0;
    let mut t = 0.0;
    let mut bands = analysis::Bands::default();
    while t < secs {
        let tick = session.tick(dt);
        bands = tick.bands;
        if let Some(mood) = args.mood {
            bands.song = demo::song_for(mood);
        }
        if let Some(p) = session.progress() {
            timeline.record(p, bands.intensity, bands.climax);
        }
        let frame = Frame {
            dt,
            bands: &bands,
            progress: session.progress().map(|p| (p * speed).min(1.0)),
            song_len: 75.0 / speed,
            playing: true,
        };
        stage.update(&frame, canvas.w, canvas.h);
        t += dt;
    }
    stage.draw(kind, &mut canvas, &palette);
    let area = Rect::new(0, 0, w, h);
    let mut buf = Buffer::empty(area);
    canvas.render(&mut buf, Rect { height: h - hud_rows, ..area }, ColorMode::True);
    if hud_rows > 0 {
        let hud = ui::Hud {
            session: &session,
            pal: &palette,
            mode: ColorMode::True,
            timeline: &timeline,
            bands: &bands,
            scene: kind,
            auto_scene: choice == SceneChoice::Auto,
            favorite: false,
            notice: None,
        };
        ui::draw(&mut buf, Rect { y: h - hud_rows, height: hud_rows, ..area }, &hud);
    }

    let mut out = String::new();
    let sgr = |c: Color, base: u8| match c {
        Color::Rgb(r, g, b) => format!("\x1b[{};2;{r};{g};{b}m", base + 8),
        _ => format!("\x1b[{}m", base + 9),
    };
    for y in 0..h {
        for x in 0..w {
            let cell = &buf[(x, y)];
            out.push_str(&sgr(cell.fg, 30));
            out.push_str(&sgr(cell.bg, 40));
            out.push_str(cell.symbol());
        }
        out.push_str("\x1b[0m\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_songs_heard_mostly_through_are_collected() {
        let track = |secs: u64, known: bool| session::TrackInfo {
            title: "x".into(),
            artist: "y".into(),
            duration: Some(Duration::from_secs(secs)),
            known,
        };
        assert!(worth_collecting(&track(200, true), 125.0));
        assert!(!worth_collecting(&track(200, true), 100.0), "menos del 60%");
        assert!(!worth_collecting(&track(200, false), 200.0), "demo o aviso");
        assert!(!worth_collecting(&track(10, true), 10.0), "demasiado corta");
        let unknown_length = session::TrackInfo { duration: None, ..track(1, true) };
        assert!(!worth_collecting(&unknown_length, 500.0));
    }

    #[test]
    fn same_path_gives_the_same_seed() {
        assert_eq!(hash_str("a/b.mp3"), hash_str("a/b.mp3"));
        assert_ne!(hash_str("a/b.mp3"), hash_str("a/c.mp3"));
    }
}

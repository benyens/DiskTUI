//! Transiciones entre canciones (y entre escenas cuando la automática cambia de escena).
//!
//! Cada una revela la escena nueva sobre la anterior con su propia forma y dibuja algo
//! encima del borde: una bandada de mariposas en diagonal, una cortina de pétalos, un iris
//! que se abre, una ola que barre la pantalla o destellos que la disuelven.

use std::f32::consts::TAU;

use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::smoothstep;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Fade,
    Butterflies,
    Petals,
    Iris,
    Wave,
    Sparkle,
}

impl Style {
    pub const ALL: [Style; 6] = [
        Style::Fade,
        Style::Butterflies,
        Style::Petals,
        Style::Iris,
        Style::Wave,
        Style::Sparkle,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Style::Fade => "fundido",
            Style::Butterflies => "mariposas",
            Style::Petals => "pétalos",
            Style::Iris => "iris",
            Style::Wave => "ola",
            Style::Sparkle => "destellos",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Style::Petals => "petalos",
            other => other.name(),
        }
    }

    /// Segundos que dura.
    pub fn seconds(self) -> f32 {
        match self {
            Style::Fade => 2.5,
            Style::Butterflies => 3.2,
            Style::Petals => 3.0,
            Style::Iris => 2.2,
            Style::Wave => 2.6,
            Style::Sparkle => 2.4,
        }
    }
}

/// Lo que elige el usuario: una transición fija o una distinta en cada canción.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Auto,
    Fixed(Style),
}

impl Choice {
    pub fn parse(name: &str) -> Option<Self> {
        let name = name.to_lowercase();
        if matches!(
            name.as_str(),
            "auto" | "automatica" | "automática" | "variada"
        ) {
            return Some(Choice::Auto);
        }
        Style::ALL
            .into_iter()
            .find(|s| s.key() == name || s.name() == name)
            .map(Choice::Fixed)
    }

    pub fn key(self) -> &'static str {
        match self {
            Choice::Auto => "auto",
            Choice::Fixed(s) => s.key(),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Choice::Auto => "una distinta en cada canción",
            Choice::Fixed(s) => s.name(),
        }
    }

    /// Avanza o retrocede: automática y luego cada estilo.
    pub fn step(self, step: i32) -> Self {
        let order: Vec<Choice> = std::iter::once(Choice::Auto)
            .chain(Style::ALL.map(Choice::Fixed))
            .collect();
        let i = order.iter().position(|c| *c == self).unwrap_or(0) as i32;
        order[(i + step).rem_euclid(order.len() as i32) as usize]
    }

    /// El estilo para la canción número `n`: en automática, uno de los vistosos (no el
    /// fundido), sin repetir el de la canción anterior.
    pub fn pick(self, n: u32, salt: u64) -> Style {
        match self {
            Choice::Fixed(s) => s,
            Choice::Auto => {
                let fancy = &Style::ALL[1..];
                let start = (salt % fancy.len() as u64) as usize;
                fancy[(start + n as usize) % fancy.len()]
            }
        }
    }
}

/// Una transición en curso.
pub struct Transition {
    pub style: Style,
    /// Avance, de 0 a 1.
    pub t: f32,
    seed: u32,
    seconds: f32,
}

fn hash(a: u32, b: u32) -> f32 {
    let h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    let h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
    (h ^ (h >> 13)) as f32 / u32::MAX as f32
}

impl Transition {
    pub fn new(style: Style, seed: u32) -> Self {
        Transition {
            style,
            t: 0.0,
            seed,
            seconds: style.seconds(),
        }
    }

    /// Con otra duración: el cambio de escena a mano con `e` es corto.
    pub fn quick(style: Style, seconds: f32) -> Self {
        Transition {
            style,
            t: 0.0,
            seed: 0,
            seconds,
        }
    }

    /// Avanza el reloj. Devuelve `false` cuando terminó.
    pub fn advance(&mut self, dt: f32) -> bool {
        self.t += dt / self.seconds.max(0.05);
        self.t < 1.0
    }

    /// Mezcla `before` (lo que se veía) con `canvas` (lo nuevo) y dibuja el adorno.
    pub fn apply(&self, canvas: &mut Canvas, before: &Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        if w < 1.0 || h < 1.0 {
            return;
        }
        let t = self.t.clamp(0.0, 1.0);
        let seed = self.seed;
        match self.style {
            Style::Fade => canvas.crossfade(before, smoothstep(0.0, 1.0, t)),
            Style::Butterflies => {
                let span = w * 0.8 + h * 0.6;
                let front = -0.12 + 1.30 * t;
                canvas.reveal(before, |x, y| {
                    let p = (0.8 * x as f32 + 0.6 * (h - y as f32)) / span;
                    let jag = 0.03 * hash(seed, (x / 3 + y / 3 * 977) as u32);
                    1.0 - smoothstep(front - 0.05, front + 0.03, p + jag)
                });
                self.draw_flock(canvas, pal, w, h, span, front);
            }
            Style::Petals => {
                let front = (-0.15 + 1.32 * t) * h;
                let edge = move |x: f32| {
                    front
                        + 0.05 * h * (x / w * 9.0 + seed as f32 * 0.01).sin()
                        + 0.025 * h * (x / w * 23.0).sin()
                };
                canvas.reveal(before, |x, y| {
                    1.0 - smoothstep(-3.0, 3.0, y as f32 - edge(x as f32))
                });
                self.draw_petals(canvas, pal, w, h, &edge);
            }
            Style::Iris => {
                let (cx, cy) = (
                    w * (0.4 + 0.2 * hash(seed, 1)),
                    h * (0.4 + 0.2 * hash(seed, 2)),
                );
                let reach = (w.max(cx * 2.0).powi(2) + h.max(cy * 2.0).powi(2)).sqrt();
                let r = smoothstep(0.0, 1.0, t) * reach * 0.75;
                canvas.reveal(before, |x, y| {
                    let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                    1.0 - smoothstep(r - 2.0, r + 2.0, d)
                });
                // Anillo de luz en el borde, que se apaga al final.
                let ring = pal.accent.mix(Rgb::WHITE, 0.5);
                let fade = 1.0 - smoothstep(0.75, 1.0, t);
                for y in 0..h as i32 {
                    for x in 0..w as i32 {
                        let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                        let k = (1.0 - (d - r).abs() / 3.0).max(0.0);
                        if k > 0.0 {
                            canvas.glow(x, y, ring, 0.5 * k * fade);
                        }
                    }
                }
            }
            Style::Wave => {
                let front = move |y: f32, t: f32| (-0.25 + 1.5 * t) * w + 0.06 * w * (y / h * 5.0 + t * 6.0).sin() + 0.015 * w * (y / h * 13.0 - t * 9.0).sin();
                canvas.reveal(before, |x, y| 1.0 - smoothstep(-2.0, 2.0, x as f32 - front(y as f32, t)));
                // El cuerpo de la ola detrás del borde, una cresta clara y espuma salpicada.
                let deep = Rgb(0.10, 0.35, 0.65).mix(pal.accent, 0.12);
                let crest = Rgb(0.55, 0.85, 1.0);
                let fade = 1.0 - smoothstep(0.8, 1.0, t);
                let body = (w * 0.16).max(3.0);
                for y in 0..h as i32 {
                    let fx = front(y as f32, t);
                    for dx in 0..body as i32 {
                        let k = 1.0 - dx as f32 / body;
                        let c = deep.mix(crest, k * k);
                        canvas.paint(fx as i32 - dx, y, c, 0.75 * k.sqrt() * fade);
                    }
                    for dx in 0..3 {
                        let foam = hash(seed, (y * 31 + dx + (t * 30.0) as i32 * 7) as u32);
                        canvas.paint(fx as i32 - dx, y, Rgb::WHITE, foam * 0.7 * fade);
                    }
                }
                for i in 0..(h * 2.0) as u32 {
                    let y = hash(seed, i * 2 + 7) * h;
                    let x = front(y, t) + 1.0 + hash(seed, i * 2 + 8).powi(2) * w * 0.04;
                    canvas.dot_over((x * 2.0) as i32, (y * 2.0) as i32, Rgb::WHITE, 0.7 * fade);
                }
            }
            Style::Sparkle => {
                // Cada bloque de 2×2 píxeles cambia en su momento, con un destello.
                let at = |x: usize, y: usize| {
                    hash(seed, (x / 2) as u32 * 7919 + (y / 2) as u32 * 104_729)
                };
                let now = -0.08 + 1.16 * t;
                canvas.reveal(before, |x, y| {
                    smoothstep(at(x, y) - 0.06, at(x, y) + 0.06, now)
                });
                let colors = [
                    Rgb::WHITE,
                    pal.accent.mix(Rgb::WHITE, 0.4),
                    pal.petal.mix(Rgb::WHITE, 0.4),
                ];
                for y in 0..h as usize {
                    for x in 0..w as usize {
                        let k = 1.0 - (at(x, y) - now).abs() / 0.03;
                        if k > 0.0 {
                            let c = colors[(at(x, y) * 1000.0) as usize % colors.len()];
                            canvas.glow(x as i32, y as i32, c, 0.55 * k);
                        }
                    }
                }
            }
        }
    }

    /// La bandada que va en el borde de la diagonal.
    fn draw_flock(
        &self,
        canvas: &mut Canvas,
        pal: &Palette,
        w: f32,
        h: f32,
        span: f32,
        front: f32,
    ) {
        let colors = [
            Rgb(1.0, 0.55, 0.12),
            pal.petal.mix(Rgb::WHITE, 0.15),
            pal.accent.mix(Rgb::WHITE, 0.1),
            Rgb(0.45, 0.75, 1.0),
            Rgb(1.0, 0.95, 0.70),
        ];
        let count = ((w + h) * 0.45).clamp(10.0, 140.0) as u32;
        for i in 0..count {
            // A lo largo del borde (perpendicular al rumbo) y algo adelante o atrás de él.
            let along = (hash(self.seed, i * 3) - 0.5) * span * 1.6;
            let ahead = (hash(self.seed, i * 3 + 1) - 0.6) * 0.18 * span;
            let a = front * span + ahead;
            let (x, y) = (0.8 * a + 0.6 * along, h - (0.6 * a - 0.8 * along));
            if x < -6.0 || y < -6.0 || x > w + 6.0 || y > h + 6.0 {
                continue;
            }
            let depth = hash(self.seed, i * 3 + 2).powf(1.4);
            let flap = self.t * 40.0 * (0.8 + 0.4 * depth) + i as f32 * 1.7;
            let wobble = 2.0 * (self.t * 9.0 + i as f32).sin();
            crate::meadow::draw_butterfly(
                canvas,
                x + wobble,
                y,
                depth,
                flap,
                colors[i as usize % colors.len()],
                0.0,
            );
        }
    }

    /// Los pétalos que caen en el borde de la cortina.
    fn draw_petals(
        &self,
        canvas: &mut Canvas,
        pal: &Palette,
        w: f32,
        h: f32,
        edge: &dyn Fn(f32) -> f32,
    ) {
        let colors = [pal.petal, pal.petal_light, Rgb(1.0, 0.80, 0.86)];
        let count = (w * 1.6) as u32;
        for i in 0..count {
            let x0 = hash(self.seed, i * 2) * w;
            let behind = hash(self.seed, i * 2 + 1);
            let sway = 3.0 * (self.t * 7.0 + i as f32 * 0.7).sin();
            let x = x0 + sway;
            let y = edge(x0) - behind * behind * h * 0.18 + 2.0;
            let c = colors[i as usize % colors.len()];
            let alpha = 1.0 - behind * 0.6;
            if i % 3 == 0 {
                // Uno de cada tres es un pétalo grande que gira: dos píxeles que rotan.
                let turn = (self.t * 8.0 + i as f32 * TAU * 0.37).sin();
                canvas.paint(x as i32, y as i32, c, alpha);
                if turn > 0.0 {
                    canvas.paint(x as i32 + 1, y as i32, c.scale(0.85), alpha);
                } else {
                    canvas.paint(x as i32, y as i32 + 1, c.scale(0.85), alpha);
                }
            } else {
                canvas.dot_over((x * 2.0) as i32, (y * 2.0) as i32, c, alpha);
                canvas.dot_over((x * 2.0) as i32 + 1, (y * 2.0) as i32, c, alpha);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(color: Rgb, w: usize, h: usize) -> Canvas {
        let mut c = Canvas::new(w, h);
        c.clear(|_, _| color);
        c
    }

    /// Proporción de píxeles que ya muestran la escena nueva (verde puro de prueba).
    fn shown(c: &Canvas) -> f32 {
        let img = c.to_image();
        let total = img.pixels().count() as f32;
        img.pixels()
            .filter(|p| p[1] > 200 && p[0] < 60 && p[2] < 60)
            .count() as f32
            / total
    }

    #[test]
    fn every_style_goes_from_old_to_new() {
        for style in Style::ALL {
            let before = scene(Rgb(1.0, 0.0, 0.0), 60, 20);
            let mut start = scene(Rgb(0.0, 1.0, 0.0), 60, 20);
            let tr = Transition {
                t: 0.0,
                ..Transition::new(style, 7)
            };
            tr.apply(&mut start, &before, &Palette::default());
            let mut end = scene(Rgb(0.0, 1.0, 0.0), 60, 20);
            let tr = Transition {
                t: 1.0,
                ..Transition::new(style, 7)
            };
            tr.apply(&mut end, &before, &Palette::default());
            assert!(
                shown(&start) < 0.1,
                "{}: empieza mostrando la nueva ({})",
                style.name(),
                shown(&start)
            );
            assert!(
                shown(&end) > 0.9,
                "{}: no termina en la nueva ({})",
                style.name(),
                shown(&end)
            );
        }
    }

    #[test]
    fn butterflies_reveal_from_the_bottom_left() {
        let before = scene(Rgb(1.0, 0.0, 0.0), 80, 30);
        let mut mid = scene(Rgb(0.0, 1.0, 0.0), 80, 30);
        Transition {
            t: 0.45,
            ..Transition::new(Style::Butterflies, 3)
        }
        .apply(&mut mid, &before, &Palette::default());
        let img = mid.to_image();
        let (w, h) = img.dimensions();
        let green = |x: u32, y: u32| img.get_pixel(x, y)[1] > 200;
        assert!(
            green(2, h - 3),
            "la esquina de abajo a la izquierda debería ser nueva"
        );
        assert!(
            !green(w - 3, 2),
            "la esquina de arriba a la derecha debería seguir vieja"
        );
    }

    #[test]
    fn auto_never_repeats_and_never_fades() {
        let mut last = None;
        for n in 0..20 {
            let s = Choice::Auto.pick(n, 12345);
            assert_ne!(s, Style::Fade);
            assert_ne!(Some(s), last);
            last = Some(s);
        }
        assert_eq!(Choice::Fixed(Style::Iris).pick(3, 1), Style::Iris);
    }

    #[test]
    fn choice_parses_and_cycles() {
        for style in Style::ALL {
            assert_eq!(Choice::parse(style.key()), Some(Choice::Fixed(style)));
        }
        assert_eq!(Choice::parse("auto"), Some(Choice::Auto));
        assert_eq!(Choice::parse("pétalos"), Some(Choice::Fixed(Style::Petals)));
        let mut c = Choice::Auto;
        for _ in 0..7 {
            c = c.step(1);
        }
        assert_eq!(c, Choice::Auto);
        assert_eq!(Choice::Auto.step(-1), Choice::Fixed(Style::Sparkle));
    }

    #[test]
    fn handles_tiny_and_mismatched_canvases() {
        for style in Style::ALL {
            for (w, h) in [(0, 0), (1, 1), (3, 2)] {
                let before = scene(Rgb::BLACK, w, h);
                let mut c = scene(Rgb::WHITE, w, h);
                Transition {
                    t: 0.5,
                    ..Transition::new(style, 1)
                }
                .apply(&mut c, &before, &Palette::default());
            }
            let before = scene(Rgb::BLACK, 10, 4);
            let mut c = scene(Rgb::WHITE, 12, 5);
            Transition {
                t: 0.5,
                ..Transition::new(style, 1)
            }
            .apply(&mut c, &before, &Palette::default());
        }
    }
}

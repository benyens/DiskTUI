//! Escena "portada": la carátula de lo que suena, en grande y en pixel art.
//!
//! La portada flota al centro sobre un fondo hecho de sus propios colores, muy borroso,
//! con un halo que late con el bombo y su reflejo en el piso. Al empezar cada canción
//! aparece pixelada y se va enfocando; durante la canción, la intensidad decide qué tan
//! fina se ve (el coro la enfoca, el verso la vuelve más gruesa). En el clímax un brillo la
//! cruza en diagonal, y en cada golpe suelta motas de sus colores. Abajo, barras que siguen
//! a graves, medios y agudos. Sin carátula (archivos sin tapa, el demo), un vinilo que gira.

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::{Cover, Palette};
use crate::plant::Rng;
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

const MAX_MOTES: usize = 160;
/// Tamaños de "píxel" de la portada, de grueso a fino: celdas por lado.
const GRIDS: [f32; 5] = [8.0, 12.0, 16.0, 24.0, 48.0];
const BARS: usize = 24;

struct Mote {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    color: Rgb,
}

pub struct Album {
    pub scenery: Scenery,
    pub cover: Option<Cover>,
    rng: Rng,
    time: f32,
    bands: Bands,
    /// Golpe de bombo, suavizado: 1 justo en el golpe.
    kick: f32,
    /// Intensidad suavizada: decide qué tan fina se ve la portada.
    focus: f32,
    /// Enfoque de entrada al empezar la canción: 0 pixelada, 1 lista.
    reveal: f32,
    /// Brillo diagonal: avance de 0 a 1 mientras cruza; `None` si no hay.
    shine: Option<f32>,
    /// Ángulo del vinilo, cuando no hay carátula.
    spin: f32,
    /// Altura de cada barra, suavizada.
    bars: [f32; BARS],
    motes: Vec<Mote>,
    /// Tamaño del lienzo en píxeles, del último cuadro: para ubicar los clicks.
    size: (f32, f32),
}

fn hash(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B1).wrapping_add(b.wrapping_mul(0x85EB_CA77)).wrapping_add(0x68E3_1DA4);
    h ^= h >> 16;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    h as f32 / u32::MAX as f32
}

/// Dónde va la portada: (esquina x, esquina y, lado), en píxeles.
fn layout(w: f32, h: f32) -> (f32, f32, f32) {
    let side = (h * 0.56).min(w * 0.45).floor().max(2.0);
    ((w - side) / 2.0, (h * 0.42 - side / 2.0).max(1.0).floor(), side)
}

impl Album {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0xA1B0_0001);
        scenery.bare = true;
        Album {
            scenery,
            cover: None,
            rng: Rng::new(seed ^ 0xA1B0_C0DE),
            time: 0.0,
            bands: Bands::default(),
            kick: 0.0,
            focus: 0.5,
            reveal: 0.0,
            shine: None,
            spin: 0.0,
            bars: [0.0; BARS],
            motes: Vec::new(),
            size: (120.0, 68.0),
        }
    }

    /// Color de la portada (o del vinilo) en (u, v) dentro del cuadrado, 0..1.
    fn art(&self, u: f32, v: f32, pal: &Palette) -> Rgb {
        match &self.cover {
            Some(cover) => cover.at(u.clamp(0.0, 0.999), v.clamp(0.0, 0.999)),
            None => self.vinyl(u, v, pal),
        }
    }

    /// Un vinilo visto de frente: surcos, etiqueta con los colores de la paleta y un
    /// reflejo que gira.
    fn vinyl(&self, u: f32, v: f32, pal: &Palette) -> Rgb {
        let (dx, dy) = (u - 0.5, v - 0.5);
        let r = (dx * dx + dy * dy).sqrt() * 2.0;
        if r > 1.0 {
            return Rgb(0.0, 0.0, 0.0);
        }
        if r < 0.36 {
            if r < 0.04 {
                return Rgb(0.05, 0.05, 0.06);
            }
            let a = dy.atan2(dx) + self.spin;
            return if a.sin() > 0.0 { pal.petal } else { pal.accent }.scale(0.85);
        }
        let groove = 0.08 + 0.03 * (r * 60.0).sin();
        let a = dy.atan2(dx) - self.spin;
        let gleam = (a * 2.0).cos().max(0.0).powi(6) * 0.25;
        Rgb(groove, groove, groove * 1.1).add(Rgb::WHITE, gleam)
    }

    /// Color promedio de la portada, para el halo.
    fn average(&self, pal: &Palette) -> Rgb {
        match &self.cover {
            Some(_) => {
                let mut sum = Rgb(0.0, 0.0, 0.0);
                for i in 0..6 {
                    for j in 0..6 {
                        sum = sum.add(self.art((i as f32 + 0.5) / 6.0, (j as f32 + 0.5) / 6.0, pal), 1.0 / 36.0);
                    }
                }
                sum
            }
            None => pal.petal,
        }
    }

    fn grid(&self) -> f32 {
        // Al entrar, de gruesa a fina; después, según la intensidad.
        let level = (self.focus * (GRIDS.len() - 1) as f32 + 0.5) as usize;
        let level = level.min(((self.reveal * GRIDS.len() as f32) as usize).min(GRIDS.len() - 1));
        GRIDS[level.clamp(1, GRIDS.len() - 1)]
    }

    fn advance(&mut self, f: &Frame) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        self.kick = (self.kick * (-dt / 0.18).exp()).max(if b.kick && f.playing { 1.0 } else { 0.0 });
        let target = if f.playing { (0.25 + b.intensity).min(1.0) } else { 0.3 };
        self.focus += (target - self.focus) * (1.0 - (-dt / 3.0).exp());
        self.reveal = (self.reveal + dt / 2.5).min(1.0);
        if f.playing {
            self.spin += dt * (0.8 + 1.2 * b.level.min(1.0));
        }
        if (b.climax || b.section) && f.playing && self.shine.is_none() {
            self.shine = Some(0.0);
        }
        if let Some(s) = &mut self.shine {
            *s += dt / 1.2;
            if *s > 1.0 {
                self.shine = None;
            }
        }
        // Barras: cada una sigue a graves, medios o agudos según dónde está.
        for (i, bar) in self.bars.iter_mut().enumerate() {
            let k = i as f32 / (BARS - 1) as f32;
            let band = if k < 0.33 { b.bass } else if k < 0.66 { b.mid } else { b.treble };
            let wobble = 0.55 + 0.45 * (self.time * (2.0 + 3.0 * hash(i as u32, 1)) + i as f32).sin();
            let want = if f.playing { (band * wobble * 1.2).min(1.0) } else { 0.0 };
            let rate = if want > *bar { 0.05 } else { 0.25 };
            *bar += (want - *bar) * (1.0 - (-dt / rate).exp());
        }
        if b.beat && f.playing {
            let n = 6 + (10.0 * b.intensity) as usize;
            for _ in 0..n {
                self.emit(None);
            }
        }
        for m in &mut self.motes {
            m.age += dt;
            m.vy -= dt * 0.01;
            m.x += m.vx * dt;
            m.y += m.vy * dt;
        }
        self.motes.retain(|m| m.age < 2.5);
        self.scenery.update(dt, b, f.progress, f.playing, 1.0, &[]);
    }

    /// Una mota desde el borde de la portada (o desde `at`, en fracciones del lienzo).
    fn emit(&mut self, at: Option<(f32, f32)>) {
        if self.motes.len() >= MAX_MOTES {
            return;
        }
        // En fracciones del cuadrado de la portada: un punto del borde.
        let t = self.rng.next() * 4.0;
        let (u, v) = match t as u32 {
            0 => (t.fract(), 0.0),
            1 => (1.0, t.fract()),
            2 => (1.0 - t.fract(), 1.0),
            _ => (0.0, 1.0 - t.fract()),
        };
        let color = match &self.cover {
            Some(c) => c.at(u.min(0.999), v.min(0.999)),
            None => Rgb(0.9, 0.85, 0.8),
        };
        let (dx, dy) = (u - 0.5, v - 0.5);
        let speed = self.rng.range(0.02, 0.08);
        let len = (dx * dx + dy * dy).sqrt().max(0.01);
        // Las coordenadas de la mota son en "lados de portada" desde la esquina.
        let (x, y) = at.unwrap_or((u, v));
        self.motes.push(Mote { x, y, vx: dx / len * speed, vy: dy / len * speed - 0.02, age: 0.0, color });
    }
}

impl Scene for Album {
    fn update(&mut self, frame: &Frame, w: usize, h: usize) {
        self.size = (w as f32, (h * 2) as f32);
        self.advance(frame);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        if w < 2.0 || h < 2.0 {
            return;
        }
        let (x0, y0, side) = layout(w, h);
        let glow = self.average(pal);
        // Fondo: la misma portada, enorme y muy borrosa, oscurecida. Se mueve tan despacio
        // que casi no cambia de un cuadro al otro (la terminal no tiene que redibujarla).
        let drift = self.time * 0.004;
        let floor_y = y0 + side;
        canvas.clear(|x, y| {
            let (fx, fy) = (x as f32, y as f32);
            let u = 0.5 + (fx - w / 2.0) / (w * 1.6) + drift.sin() * 0.05;
            let v = 0.5 + (fy - h / 2.0) / (h * 1.6) + drift.cos() * 0.05;
            let mut c = Rgb(0.0, 0.0, 0.0);
            if self.cover.is_some() {
                for (du, dv) in [(-0.12, -0.12), (0.12, -0.12), (-0.12, 0.12), (0.12, 0.12)] {
                    c = c.add(self.art(u + du, v + dv, pal), 0.25);
                }
            } else {
                // Sin carátula, un degradé suave con los colores de la paleta.
                c = pal.petal.mix(pal.accent, (u + v * 0.5).clamp(0.0, 1.0)).scale(0.7);
            }
            let r = (((fx - w / 2.0) / w).powi(2) + ((fy - h * 0.42) / h).powi(2)).sqrt();
            let floor = if fy > floor_y { 0.75 } else { 1.0 };
            c.scale((0.30 - 0.25 * r).max(0.05) * floor)
        });
        // Halo que late con el bombo.
        let halo = 0.18 + 0.35 * self.kick;
        let reach = (side * 0.12).max(2.0);
        for y in (y0 - reach) as i32..(y0 + side + reach) as i32 {
            for x in (x0 - reach) as i32..(x0 + side + reach) as i32 {
                let dx = (x0 - x as f32).max(x as f32 - (x0 + side)).max(0.0);
                let dy = (y0 - y as f32).max(y as f32 - (y0 + side)).max(0.0);
                let d = (dx * dx + dy * dy).sqrt();
                if d > 0.0 && d < reach {
                    canvas.paint(x, y, glow.add(Rgb::WHITE, 0.2), halo * (1.0 - d / reach).powi(2));
                }
            }
        }
        // La portada, en pixel art: se muestrea en una grilla y cada celda va de un color.
        let grid = self.grid();
        let shine = self.shine;
        let pixel = |x: f32, y: f32| -> Option<Rgb> {
            let (u, v) = ((x - x0) / side, (y - y0) / side);
            if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                return None;
            }
            let (gu, gv) = (((u * grid).floor() + 0.5) / grid, ((v * grid).floor() + 0.5) / grid);
            let mut c = self.art(gu, gv, pal);
            if let Some(s) = shine {
                let band = (u + v) * 0.5 - (s * 1.6 - 0.3);
                if band.abs() < 0.06 {
                    c = c.add(Rgb::WHITE, 0.4 * (1.0 - band.abs() / 0.06));
                }
            }
            Some(c)
        };
        for y in y0 as i32..(y0 + side) as i32 {
            for x in x0 as i32..(x0 + side) as i32 {
                if let Some(c) = pixel(x as f32 + 0.5, y as f32 + 0.5) {
                    canvas.solid(x, y, c);
                }
            }
        }
        // Reflejo en el piso: la portada al revés, que se apaga.
        let depth = (side * 0.35).min(h - floor_y - 1.0).max(0.0);
        for dy in 0..depth as i32 {
            let k = dy as f32 / depth.max(1.0);
            let src = floor_y - 1.0 - dy as f32;
            for x in x0 as i32..(x0 + side) as i32 {
                if let Some(c) = pixel(x as f32 + 0.5, src) {
                    canvas.paint(x, floor_y as i32 + dy, c.scale(0.6), 0.55 * (1.0 - k));
                }
            }
        }
        // Barras abajo, de un lado al otro, con colores de la portada.
        let base = h - 1.0;
        let top_room = (h - floor_y - depth * 0.4).max(2.0);
        let bar_w = w / BARS as f32;
        for (i, height) in self.bars.iter().enumerate() {
            let tall = (height * top_room * 0.8).max(0.0);
            let k = (i as f32 + 0.5) / BARS as f32;
            let c = match &self.cover {
                Some(cover) => cover.at(k, 0.85),
                None => pal.petal.mix(pal.accent, k),
            }
            .mix(Rgb::WHITE, 0.25);
            let (bx0, bx1) = ((i as f32 * bar_w + 1.0) as i32, ((i as f32 + 1.0) * bar_w - 1.0) as i32);
            for y in (base - tall) as i32..=base as i32 {
                for x in bx0..bx1.max(bx0 + 1) {
                    canvas.paint(x, y, c, 0.55);
                }
            }
        }
        // Motas: en "lados de portada" desde la esquina de la portada.
        for m in &self.motes {
            let a = (1.0 - m.age / 2.5) * 0.9;
            canvas.paint((x0 + m.x * side) as i32, (y0 + m.y * side) as i32, m.color.add(Rgb::WHITE, 0.2), a);
        }
    }

    fn new_track(&mut self) {
        self.reveal = 0.0;
        self.shine = None;
        self.motes.clear();
        self.scenery.fade();
    }

    /// Un click hace brillar la portada y suelta una nube de motas ahí.
    fn poke(&mut self, x: f32, y: f32) {
        self.shine = Some(0.0);
        // El click viene en fracciones del lienzo: se pasa a "lados de portada".
        let (w, h) = self.size;
        let (x0, y0, side) = layout(w, h);
        let at = ((x * w - x0) / side, (y * h - y0) / side);
        for _ in 0..30 {
            self.emit(Some(at));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(a: &mut Album, bands: &Bands, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            a.update(&Frame { dt: 1.0 / 60.0, bands, progress: Some(0.5), song_len: 180.0, playing: true }, 120, 34);
        }
    }

    fn red_cover() -> Cover {
        let mut png = Vec::new();
        let img = image::RgbImage::from_fn(8, 8, |x, _| if x < 4 { image::Rgb([220, 30, 30]) } else { image::Rgb([30, 30, 220]) });
        image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        Cover::from_image_bytes(&png).unwrap()
    }

    #[test]
    fn the_cover_comes_into_focus_and_follows_the_intensity() {
        let mut a = Album::new(1);
        a.new_track();
        assert_eq!(a.grid(), GRIDS[1], "empieza pixelada");
        run(&mut a, &Bands { intensity: 1.0, ..Bands::default() }, 8.0);
        assert_eq!(a.grid(), GRIDS[GRIDS.len() - 1], "con el coro, fina");
        run(&mut a, &Bands { intensity: 0.0, ..Bands::default() }, 12.0);
        assert!(a.grid() < GRIDS[GRIDS.len() - 1], "en calma, más gruesa");
    }

    #[test]
    fn the_cover_is_drawn_in_the_middle() {
        let mut a = Album::new(2);
        a.cover = Some(red_cover());
        run(&mut a, &Bands::default(), 3.0);
        let mut canvas = Canvas::new(100, 30);
        a.draw(&mut canvas, &Palette::default());
        let (x0, y0, side) = layout(100.0, 60.0);
        let left = canvas.pixel((x0 + side * 0.2) as i32, (y0 + side * 0.5) as i32).unwrap();
        let right = canvas.pixel((x0 + side * 0.8) as i32, (y0 + side * 0.5) as i32).unwrap();
        assert!(left.0 > 0.6 && left.2 < 0.3, "{left:?}");
        assert!(right.2 > 0.6 && right.0 < 0.3, "{right:?}");
    }

    #[test]
    fn climax_shines_and_beats_throw_motes() {
        let mut a = Album::new(3);
        run(&mut a, &Bands { climax: true, beat: true, intensity: 0.5, ..Bands::default() }, 1.0 / 60.0);
        assert!(a.shine.is_some());
        assert!(!a.motes.is_empty());
        run(&mut a, &Bands::default(), 3.0);
        assert!(a.shine.is_none() && a.motes.is_empty());
    }

    #[test]
    fn draws_at_any_size_with_and_without_cover() {
        for cover in [None, Some(red_cover())] {
            let mut a = Album::new(4);
            a.cover = cover;
            let bands = Bands { level: 1.0, bass: 1.0, mid: 1.0, treble: 1.0, kick: true, beat: true, climax: true, ..Bands::default() };
            run(&mut a, &bands, 1.0);
            a.poke(0.5, 0.5);
            for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (220, 60)] {
                let mut canvas = Canvas::new(w, h);
                a.draw(&mut canvas, &Palette::default());
            }
        }
    }
}

//! Escena "globos": globos aerostáticos que suben sobre un valle de campos de colores.
//!
//! Cada globo es una gota de gajos de colores con su canasta. En cada pulso alguno enciende
//! el quemador: la llama ilumina la tela por dentro y el globo sube un poco; entre pulsos
//! baja despacio. A medida que avanza la canción todos van ganando altura. En el clímax
//! encienden todos a la vez. El viento los lleva de lado y, al salir por un borde, vuelven
//! por el otro. Dos globos llevan los colores de la carátula.

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::Rng;
use crate::render::View;
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// Dónde empieza el valle, como fracción del alto.
const HORIZON: f32 = 0.64;
const COUNT: usize = 6;
const MAX_BALLOONS: usize = 10;
/// Cuánto baja la gota por debajo del centro, en radios: ahí está la boca.
const THROAT: f32 = 1.3;

fn hash(a: u32, b: u32) -> f32 {
    let h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    let h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
    (h ^ (h >> 13)) as f32 / u32::MAX as f32
}

/// Medio ancho de la gota a la altura `dy` (en radios, 0 en el centro, positivo hacia abajo).
fn half_width(dy: f32) -> f32 {
    if !(-1.0..=THROAT).contains(&dy) {
        0.0
    } else if dy <= 0.0 {
        (1.0 - dy * dy).sqrt()
    } else {
        let k = dy / THROAT;
        (1.0 - k * k).max(0.0).sqrt() * (1.0 - 0.55 * k) + 0.12 * k
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Pattern {
    /// Gajos alternados.
    Gores,
    /// Gajos de un color con una franja ancha al medio.
    Band,
    /// Gajos y franjas cruzados: un tablero.
    Checks,
}

struct Balloon {
    /// Posición del centro de la gota: x en fracción del ancho, y en fracción del alto.
    x: f32,
    y: f32,
    vy: f32,
    /// Tamaño y cercanía: los chicos están lejos y se mueven más lento.
    depth: f32,
    colors: [Rgb; 2],
    /// Los que llevan los colores de la carátula: se leen de la paleta al dibujar.
    cover: Option<bool>,
    pattern: Pattern,
    flame: f32,
    /// Cuánto sube este globo respecto de los demás.
    lift: f32,
}

pub struct Balloons {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    progress: f32,
    bands: Bands,
    balloons: Vec<Balloon>,
    seed: u32,
}

const FIXED: [[Rgb; 2]; 6] = [
    [Rgb(0.88, 0.22, 0.20), Rgb(0.98, 0.95, 0.88)],
    [Rgb(0.98, 0.78, 0.20), Rgb(0.20, 0.45, 0.80)],
    [Rgb(0.30, 0.70, 0.45), Rgb(0.98, 0.90, 0.40)],
    [Rgb(0.60, 0.35, 0.80), Rgb(0.98, 0.60, 0.70)],
    [Rgb(0.95, 0.50, 0.15), Rgb(0.25, 0.20, 0.35)],
    [Rgb(0.20, 0.65, 0.80), Rgb(0.98, 0.98, 0.98)],
];

impl Balloons {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0xBA11_0001);
        scenery.bare = true;
        let mut rng = Rng::new(seed ^ 0xBA11_F1AE);
        let seed = (rng.next() * 1e6) as u32;
        let mut b = Balloons {
            scenery,
            rng,
            time: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            balloons: Vec::new(),
            seed,
        };
        b.launch_all();
        b
    }

    fn make(&mut self, x: f32, y: f32, depth: f32, n: usize) -> Balloon {
        let colors = FIXED[(self.rng.next() * FIXED.len() as f32) as usize % FIXED.len()];
        let cover = if n < 2 { Some(n == 1) } else { None };
        let pattern = match (self.rng.next() * 3.0) as u32 {
            0 => Pattern::Gores,
            1 => Pattern::Band,
            _ => Pattern::Checks,
        };
        Balloon { x, y, vy: 0.0, depth, colors, cover, pattern, flame: 0.0, lift: self.rng.range(-0.08, 0.08) }
    }

    fn launch_all(&mut self) {
        self.balloons.clear();
        for n in 0..COUNT {
            let depth = 0.45 + 0.75 * (n as f32 / (COUNT - 1) as f32) + self.rng.range(-0.08, 0.08);
            let x = (n as f32 + self.rng.range(0.1, 0.9)) / COUNT as f32;
            let y = self.rng.range(0.30, 0.55);
            let b = self.make(x, y, depth, n);
            self.balloons.push(b);
        }
        // Del más lejano al más cercano: así se dibujan en orden.
        self.balloons.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    }

    /// Altura a la que tiende un globo: más arriba a medida que avanza la canción.
    fn cruise(&self, b: &Balloon) -> f32 {
        0.52 - 0.28 * self.progress - 0.10 * (b.depth - 0.8) + b.lift
    }

    fn fire(b: &mut Balloon, strength: f32) {
        b.flame = 1.0;
        b.vy -= 0.06 * strength;
    }

    fn advance(&mut self, f: &Frame) {
        let (dt, bands) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *bands;
        if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }
        if f.playing && !self.balloons.is_empty() {
            if bands.climax {
                for b in &mut self.balloons {
                    Self::fire(b, 1.5);
                }
            } else if bands.beat {
                let i = (self.rng.next() * self.balloons.len() as f32) as usize % self.balloons.len();
                Self::fire(&mut self.balloons[i], 1.0);
            }
        }
        let wind = self.scenery.wind();
        let targets: Vec<f32> = self.balloons.iter().map(|b| self.cruise(b)).collect();
        for (b, target) in self.balloons.iter_mut().zip(targets) {
            b.flame *= (-dt / 0.35).exp();
            // Baja despacio, con un resorte suave hacia su altura de crucero.
            b.vy += dt * (0.012 + 0.05 * (target - b.y));
            b.vy *= (-dt * 0.8).exp();
            b.y = (b.y + b.vy * dt).clamp(0.08, 0.9);
            b.x += dt * (0.006 + 0.02 * wind) * b.depth;
            if b.x > 1.15 {
                b.x -= 1.3;
            }
        }
        // Los globos lanzados con un click que ya subieron mucho se van.
        while self.balloons.len() > COUNT && self.balloons.iter().any(|b| b.y <= 0.081) {
            if let Some(i) = self.balloons.iter().position(|b| b.y <= 0.081) {
                self.balloons.remove(i);
            }
        }
        self.scenery.update(dt, bands, Some(self.progress), f.playing, 1.0, &[]);
    }

    /// El valle: cerros azulados al fondo y franjas de cultivos hacia adelante, con un río.
    fn draw_valley(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let atm = self.scenery.atmosphere();
        let day = atm.daylight;
        let horizon = h * HORIZON;
        let haze = self.scenery.sky(1.0);
        let s = self.seed as f32 * 0.001;
        // Cerros lejanos.
        for x in 0..canvas.w as i32 {
            let u = x as f32 / h;
            let top = horizon - h * (0.05 + 0.03 * (u * 3.0 + s).sin() + 0.02 * (u * 7.0 - s).sin());
            for y in top as i32..horizon as i32 + 1 {
                canvas.solid(x, y, Rgb(0.30, 0.40, 0.55).mix(haze, 0.5).scale(0.5 + 0.5 * day));
            }
        }
        // Campos: franjas cuya separación crece hacia adelante (perspectiva), cortadas en
        // parcelas de colores. El borde de cada franja ondula como lomas.
        let crops = [
            Rgb(0.36, 0.58, 0.25),
            Rgb(0.55, 0.68, 0.28),
            Rgb(0.82, 0.70, 0.30),
            Rgb(0.62, 0.50, 0.75),
            Rgb(0.28, 0.48, 0.24),
            Rgb(0.75, 0.55, 0.30),
        ];
        let river = |y: f32| w * (0.62 + 0.18 * ((y - horizon) / h * 9.0 + s).sin()) - (y - horizon) * 0.4;
        for y in horizon as i32..h as i32 {
            let v = (y as f32 - horizon) / (h - horizon).max(1.0);
            // Fila de campo: más fina cerca del horizonte.
            let row = (v.sqrt() * 9.0) as u32;
            let width = 0.5 + 6.0 * v;
            let mist = (1.0 - v).powi(3) * 0.5;
            let rx = river(y as f32);
            for x in 0..canvas.w as i32 {
                let fx = x as f32;
                let wave = (fx / h * 4.0 + row as f32 * 1.7).sin() * 0.5;
                let lot = ((fx / (w * (0.12 + 0.1 * hash(row, 3))) + wave) as i32 as u32).wrapping_add(row * 13);
                let mut c = crops[(hash(lot ^ self.seed, row) * crops.len() as f32) as usize % crops.len()];
                // Surcos y setos entre parcelas.
                if ((fx / (w * (0.12 + 0.1 * hash(row, 3))) + wave).fract()) < 0.04 + 0.02 * (1.0 - v) {
                    c = Rgb(0.18, 0.30, 0.15);
                }
                if v > 0.25 && (y as u32 + lot).is_multiple_of(3) {
                    c = c.scale(0.92);
                }
                // Río que serpentea y refleja el cielo.
                if (fx - rx).abs() < width {
                    c = self.scenery.sky(0.6).mix(Rgb(0.45, 0.65, 0.80), 0.4).add(Rgb::WHITE, 0.15 * day);
                }
                let lit = c.scale(0.45 + 0.55 * day).mix(pal.leaf, 0.06);
                canvas.solid(x, y, lit.mix(haze, mist));
            }
        }
        // Árboles sueltos en los bordes de los campos.
        for i in 0..(w * 0.25) as u32 {
            let x = hash(i ^ self.seed, 1) * w;
            let v = hash(i ^ self.seed, 2).powi(2);
            let y = horizon + v * (h - horizon);
            let r = (0.5 + 2.5 * v) as i32;
            let c = Rgb(0.12, 0.28, 0.14).scale(0.5 + 0.5 * day);
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy <= r * r {
                        canvas.solid(x as i32 + dx, y as i32 + dy - r, c);
                    }
                }
            }
        }
    }

    fn draw_balloon(&self, canvas: &mut Canvas, b: &Balloon, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let atm = self.scenery.atmosphere();
        let day = 0.45 + 0.55 * atm.daylight;
        let r = (h * 0.095 * b.depth).max(2.0);
        let (cx, cy) = (b.x * w, b.y * h);
        // La luz viene del lado del sol (o la luna).
        let side = if atm.daylight > 0.4 { self.scenery.sun_x() } else { self.scenery.moon_x() } - b.x;
        let side = side.signum();
        // Los lejanos se ven más pálidos, metidos en el aire.
        let haze = self.scenery.sky(0.7);
        let far = (1.0 - b.depth).max(0.0) * 0.5;
        let flame = b.flame;
        // Con globos chicos, menos gajos y sin tablero: si no, se ve ruido.
        let gores = if r >= 9.0 { 8.0 } else { 6.0 };
        let pattern = if r < 8.0 && b.pattern == Pattern::Checks { Pattern::Gores } else { b.pattern };
        let colors = match b.cover {
            Some(false) => [pal.petal, pal.petal_light.mix(Rgb::WHITE, 0.3)],
            Some(true) => [pal.accent, pal.petal],
            None => b.colors,
        };
        for py in (cy - r) as i32 - 1..=(cy + r * THROAT) as i32 + 1 {
            let dy = (py as f32 + 0.5 - cy) / r;
            let half = half_width(dy);
            if half <= 0.0 {
                continue;
            }
            for px in (cx - half * r) as i32 - 1..=(cx + half * r) as i32 + 1 {
                let dx = (px as f32 + 0.5 - cx) / r;
                if dx.abs() > half {
                    continue;
                }
                let u = (dx / half).clamp(-1.0, 1.0);
                let gore = ((u.asin() / std::f32::consts::PI + 0.5) * gores) as i32;
                let band = ((dy + 1.0) / (1.0 + THROAT) * if r >= 9.0 { 5.0 } else { 4.0 }) as i32;
                let pick = match pattern {
                    Pattern::Gores => gore % 2,
                    Pattern::Band => i32::from(band == 2),
                    Pattern::Checks => (gore + band) % 2,
                };
                let mut c = colors[pick as usize];
                // Volumen: más claro del lado de la luz, oscuro en el borde opuesto y abajo.
                let light = 0.70 + 0.30 * u * side - 0.25 * (dy / THROAT).max(0.0) - 0.15 * (1.0 - (1.0 - u * u).sqrt());
                c = c.scale(light * day);
                // Brillo arriba, del lado de la luz.
                if dy < -0.45 && u * side > 0.2 && u * side < 0.6 {
                    c = c.add(Rgb::WHITE, 0.18 * atm.daylight);
                }
                // La llama ilumina la tela desde adentro, sobre todo abajo.
                if flame > 0.01 {
                    let warm = flame * (0.3 + 0.7 * ((dy + 0.3) / (THROAT + 0.3)).clamp(0.0, 1.0));
                    c = c.add(Rgb(1.0, 0.65, 0.25), 0.5 * warm);
                }
                canvas.solid(px, py, c.mix(haze, far));
            }
        }
        // Canasta y cuerdas.
        let throat_y = cy + r * THROAT;
        let basket_y = throat_y + (r * 0.45).max(1.5);
        let bw = (r * 0.28).max(1.0);
        let rope = Rgb(0.25, 0.20, 0.15).mix(haze, far);
        let steps = (basket_y - throat_y).ceil() as i32;
        for s in 0..=steps {
            let k = s as f32 / steps.max(1) as f32;
            let y = throat_y + (basket_y - throat_y) * k;
            for edge in [-1.0, 1.0] {
                let x = cx + edge * (r * 0.16 * (1.0 - k) + bw * k);
                canvas.solid(x as i32, y as i32, rope);
            }
        }
        let basket = Rgb(0.45, 0.30, 0.15).scale(day).mix(haze, far);
        for y in basket_y as i32..(basket_y + (r * 0.3).max(1.0)) as i32 + 1 {
            for x in (cx - bw) as i32..=(cx + bw) as i32 {
                canvas.solid(x, y, basket);
            }
        }
        // La llama del quemador, entre la boca y la canasta.
        if flame > 0.05 {
            let tall = ((basket_y - throat_y) * flame).max(1.0);
            for k in 0..tall as i32 {
                let y = basket_y as i32 - 1 - k;
                let c = Rgb(1.0, 0.95, 0.6).mix(Rgb(1.0, 0.45, 0.1), k as f32 / tall);
                canvas.solid(cx as i32, y, c);
            }
            canvas.glow(cx as i32, (basket_y - tall / 2.0) as i32, Rgb(1.0, 0.7, 0.3), 0.35 * flame * (0.5 + b.depth));
        }
    }
}

impl Scene for Balloons {
    fn update(&mut self, frame: &Frame, _w: usize, _h: usize) {
        self.advance(frame);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        if w < 2.0 || h < 2.0 {
            return;
        }
        let horizon = (h * HORIZON).floor();
        let view = View { cx: w / 2.0, ground: horizon, scale: horizon.max(4.0), weight: 1.0 };
        let scenery = &self.scenery;
        canvas.clear(|_, y| scenery.sky((y as f32 / horizon).min(1.0)).mix(pal.bg_bottom, 0.05));
        self.scenery.draw_back(canvas, &view, pal);
        self.draw_valley(canvas, pal);
        for b in &self.balloons {
            self.draw_balloon(canvas, b, pal);
        }
    }

    fn new_track(&mut self) {
        // Globos nuevos con los colores de la canción nueva (los tapa la transición).
        self.seed = (self.rng.next() * 1e6) as u32;
        self.launch_all();
        self.scenery.fade();
    }

    /// Un click lanza un globo nuevo desde ahí (o hace subir el más cercano, si ya hay muchos).
    fn poke(&mut self, x: f32, y: f32) {
        if self.balloons.len() < MAX_BALLOONS {
            let n = self.balloons.len();
            let depth = self.rng.range(0.8, 1.2);
            let mut b = self.make(x, y.clamp(0.15, 0.85), depth, n);
            Self::fire(&mut b, 2.0);
            b.lift = -0.5;
            let at = self.balloons.partition_point(|o| o.depth < b.depth);
            self.balloons.insert(at, b);
        } else if let Some(b) = self.balloons.iter_mut().min_by(|a, b| ((a.x - x).abs() + (a.y - y).abs()).total_cmp(&((b.x - x).abs() + (b.y - y).abs()))) {
            Self::fire(b, 2.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &mut Balloons, bands: &Bands, progress: f32, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            s.update(&Frame { dt: 1.0 / 60.0, bands, progress: Some(progress), song_len: 180.0, playing: true }, 120, 34);
        }
    }

    fn mean_height(s: &Balloons) -> f32 {
        s.balloons.iter().map(|b| b.y).sum::<f32>() / s.balloons.len() as f32
    }

    #[test]
    fn the_shape_is_a_drop_with_a_narrow_mouth() {
        assert!((half_width(-1.0)).abs() < 1e-4);
        assert!((half_width(0.0) - 1.0).abs() < 1e-4);
        assert!(half_width(1.0) < 0.5 && half_width(1.0) > 0.0);
        assert_eq!(half_width(THROAT + 0.1), 0.0);
    }

    #[test]
    fn they_rise_as_the_song_goes_on() {
        let mut early = Balloons::new(1);
        let mut late = Balloons::new(1);
        run(&mut early, &Bands::default(), 0.05, 40.0);
        run(&mut late, &Bands::default(), 0.95, 40.0);
        // y crece hacia abajo: más arriba es menor.
        assert!(mean_height(&late) < mean_height(&early) - 0.15, "{} {}", mean_height(&late), mean_height(&early));
    }

    #[test]
    fn a_beat_fires_a_burner_and_the_climax_fires_them_all() {
        let mut s = Balloons::new(2);
        run(&mut s, &Bands { beat: true, ..Bands::default() }, 0.5, 1.0 / 60.0);
        assert_eq!(s.balloons.iter().filter(|b| b.flame > 0.9).count(), 1);
        run(&mut s, &Bands { climax: true, ..Bands::default() }, 0.5, 1.0 / 60.0);
        assert!(s.balloons.iter().all(|b| b.flame > 0.9));
        let before = mean_height(&s);
        run(&mut s, &Bands::default(), 0.5, 0.5);
        assert!(mean_height(&s) < before, "suben con la llama");
    }

    #[test]
    fn a_click_launches_a_balloon_there_up_to_a_limit() {
        let mut s = Balloons::new(3);
        s.poke(0.3, 0.7);
        assert_eq!(s.balloons.len(), COUNT + 1);
        assert!(s.balloons.iter().any(|b| (b.x - 0.3).abs() < 1e-4 && b.flame > 0.9));
        for _ in 0..20 {
            s.poke(0.5, 0.5);
        }
        assert_eq!(s.balloons.len(), MAX_BALLOONS);
        assert!(s.balloons.windows(2).all(|p| p[0].depth <= p[1].depth), "ordenados por cercanía");
    }

    #[test]
    fn draws_at_any_size() {
        let mut s = Balloons::new(4);
        let bands = Bands { level: 1.0, intensity: 1.0, beat: true, climax: true, ..Bands::default() };
        run(&mut s, &bands, 0.7, 1.0);
        s.poke(0.5, 0.5);
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (220, 60)] {
            let mut canvas = Canvas::new(w, h);
            s.draw(&mut canvas, &Palette::default());
        }
    }
}

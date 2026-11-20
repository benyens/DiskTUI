//! Escena "nieve": un pueblo de montaña bajo la nieve, de noche.
//!
//! Casas con techos nevados en fila, pinos entre ellas y una iglesia al medio. Las
//! ventanas se van encendiendo a medida que avanza la canción, como un pueblo que despierta;
//! en cada golpe alguna titila. La nieve cae en tres capas, más tupida con la intensidad, y
//! se va juntando en techos y suelo durante la canción. En el clímax suena la campana: la
//! ventana del campanario brilla y salen ondas. Las chimeneas echan humo que lleva el viento.
//!
//! El pueblo no se guarda en ningún lado: cada franja del ancho ("solar") decide con un hash
//! si tiene casa, pino o la iglesia, así que se adapta solo a cualquier tamaño de terminal.

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::Rng;
use crate::render::View;
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// Dónde está el suelo del pueblo, como fracción del alto.
const GROUND: f32 = 0.79;
const MAX_TWINKLES: usize = 24;
const MAX_POOF: usize = 120;

/// Número pseudoazar en 0..1 a partir de dos enteros (mezclador de murmur3: con entradas
/// parecidas, como solares vecinos, da resultados bien distintos).
fn hash(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B1).wrapping_add(b.wrapping_mul(0x85EB_CA77)).wrapping_add(0x68E3_1DA4);
    h ^= h >> 16;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    h as f32 / u32::MAX as f32
}

/// Qué hay en cada solar del pueblo.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Lot {
    House,
    Pine,
    Church,
}

/// Una casa ya medida en píxeles.
struct House {
    x: f32,
    width: f32,
    wall: f32,
    roof: f32,
    color: Rgb,
    chimney: bool,
    cols: usize,
    rows: usize,
}

struct Twinkle {
    lot: usize,
    window: usize,
    age: f32,
}

struct Flake {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
}

pub struct Village {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    progress: f32,
    bands: Bands,
    /// Campana: 1 justo al sonar, baja sola.
    bell: f32,
    twinkles: Vec<Twinkle>,
    /// Casas que se encendieron enteras con un click: (solar, edad).
    woken: Vec<(usize, f32)>,
    poof: Vec<Flake>,
    /// Tamaño del lienzo en píxeles, del último cuadro: para ubicar los clicks.
    size: (f32, f32),
    seed: u32,
}

impl Village {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0x5110_0001);
        scenery.bare = true;
        let mut rng = Rng::new(seed ^ 0x5110_BE11);
        let seed = (rng.next() * 1e6) as u32;
        Village {
            scenery,
            rng,
            time: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            bell: 0.0,
            twinkles: Vec::new(),
            woken: Vec::new(),
            poof: Vec::new(),
            size: (120.0, 68.0),
            seed,
        }
    }

    /// Ancho de cada solar, en píxeles.
    fn lot_width(h: f32) -> f32 {
        (h * 0.22).max(8.0)
    }

    fn lots(w: f32, h: f32) -> usize {
        (w / Self::lot_width(h)).ceil() as usize + 1
    }

    /// La iglesia va en el solar del medio; el resto, casas y algunos pinos.
    fn lot(&self, i: usize, count: usize) -> Lot {
        if i == count / 2 {
            Lot::Church
        } else if hash(self.seed ^ 0xA1, i as u32) < 0.28 {
            Lot::Pine
        } else {
            Lot::House
        }
    }

    fn house(&self, i: usize, h: f32) -> House {
        let lw = Self::lot_width(h);
        let r = |k: u32| hash(self.seed ^ (i as u32).wrapping_mul(31), k);
        let width = lw * (0.62 + 0.25 * r(1));
        let wall = h * (0.09 + 0.06 * r(2));
        let walls = [Rgb(0.42, 0.20, 0.14), Rgb(0.28, 0.24, 0.34), Rgb(0.44, 0.33, 0.20), Rgb(0.22, 0.30, 0.26), Rgb(0.46, 0.40, 0.34)];
        let cols = ((width / 5.5) as usize).clamp(1, 4);
        House {
            x: i as f32 * lw + (lw - width) * r(3),
            width,
            wall,
            roof: width * (0.32 + 0.15 * r(4)),
            color: walls[(r(5) * walls.len() as f32) as usize % walls.len()],
            chimney: r(6) < 0.65,
            cols,
            rows: if wall > 12.0 { 2 } else { 1 },
        }
    }

    /// ¿Está encendida esta ventana? Cada vez más a medida que avanza la canción.
    fn lit(&self, lot: usize, window: usize) -> bool {
        hash(self.seed ^ (lot as u32).wrapping_mul(977), window as u32) < 0.15 + 0.75 * self.progress
            || self.woken.iter().any(|&(l, _)| l == lot)
    }

    /// Cuánta nieve se ha juntado (0 a 1): crece con la canción.
    fn snowpack(&self) -> f32 {
        0.3 + 0.7 * self.progress
    }

    fn advance(&mut self, f: &Frame, w: usize, h: usize) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }
        self.size = (w as f32, (h * 2) as f32);
        self.bell *= (-dt / 2.5).exp();
        if b.climax && f.playing {
            self.bell = 1.0;
        }
        for t in &mut self.twinkles {
            t.age += dt;
        }
        self.twinkles.retain(|t| t.age < 0.6);
        if b.beat && f.playing && self.twinkles.len() < MAX_TWINKLES {
            let lots = Self::lots(self.size.0, self.size.1);
            let lot = (self.rng.next() * lots as f32) as usize;
            let window = (self.rng.next() * 8.0) as usize;
            self.twinkles.push(Twinkle { lot, window, age: 0.0 });
        }
        for w in &mut self.woken {
            w.1 += dt;
        }
        self.woken.retain(|w| w.1 < 12.0);
        for p in &mut self.poof {
            p.age += dt;
            p.vy += dt * 0.25;
            p.vx *= (-dt * 2.0).exp();
            p.x += p.vx * dt;
            p.y += p.vy * dt;
        }
        self.poof.retain(|p| p.age < 1.5);
        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    fn snow_color(&self) -> Rgb {
        let atm = self.scenery.atmosphere();
        Rgb(0.78, 0.85, 1.0).scale(0.55 + 0.45 * atm.daylight).mix(self.scenery.sky(0.2), 0.10)
    }

    /// Dos cordones de montañas nevadas detrás del pueblo.
    fn draw_mountains(&self, canvas: &mut Canvas) {
        let h = canvas.px_h() as f32;
        let snow = self.snow_color();
        let day = self.scenery.atmosphere().daylight;
        for (layer, (base, amp, tint)) in [(0.52, 0.16, 0.35), (0.64, 0.10, 0.6)].into_iter().enumerate() {
            let s = (self.seed as f32 * 0.001) + layer as f32 * 3.7;
            let rock = self.scenery.sky(1.0).mix(Rgb(0.22, 0.26, 0.36), 0.5).mix(Rgb(0.08, 0.09, 0.13), tint).scale(0.6 + 0.4 * day);
            for x in 0..canvas.w as i32 {
                let u = x as f32 / h;
                let ridge = (u * 4.3 + s).sin() * 0.5 + (u * 9.1 + s * 2.0).sin() * 0.3 + (1.0 - (u * 17.0 - s).sin().abs()) * 0.2;
                let top = h * (base - amp * ridge);
                let peak = h * (base - amp * 0.9);
                for y in top.ceil() as i32..(h * GROUND) as i32 + 1 {
                    // Nieve en lo alto y en las laderas que miran a la luz.
                    let height = ((y as f32 - peak) / (h * amp * 1.4)).clamp(0.0, 1.0);
                    let capped = height < 0.55 + 0.12 * (x as f32 * 0.9 + y as f32 * 1.3).sin() || (y as f32 - top) < 1.5;
                    let c = if capped { snow.mix(rock, 0.2 + 0.25 * layer as f32) } else { rock };
                    canvas.solid(x, y, c);
                }
            }
        }
    }

    fn draw_pine(&self, canvas: &mut Canvas, cx: f32, ground: f32, size: f32) {
        let snow = self.snow_color();
        let needles = Rgb(0.06, 0.16, 0.12).scale(0.6 + 0.5 * self.scenery.atmosphere().daylight);
        let tiers = 3;
        for t in 0..tiers {
            let tier_h = size * 0.42;
            let top = ground - size + t as f32 * size * 0.26;
            for dy in 0..tier_h as i32 {
                let half = (dy as f32 / tier_h) * size * (0.22 + 0.06 * t as f32);
                for dx in -(half as i32)..=half as i32 {
                    // Nieve en el borde de arriba de cada piso.
                    let edge = dy < 2 || (dx.abs() as f32) > half - 1.2 && dy % 3 == 0;
                    let c = if edge { snow.scale(0.9) } else { needles };
                    canvas.solid((cx + dx as f32) as i32, (top + dy as f32) as i32, c);
                }
            }
        }
        for y in (ground - size * 0.15) as i32..ground as i32 {
            canvas.solid(cx as i32, y, Rgb(0.18, 0.11, 0.07));
        }
    }

    fn window_color(&self, pal: &Palette, glow: f32) -> Rgb {
        Rgb(1.0, 0.76, 0.38).mix(pal.petal_light, 0.18).scale(0.75 + 0.25 * glow)
    }

    fn draw_house(&self, canvas: &mut Canvas, pal: &Palette, lot: usize, house: &House, ground: f32) {
        let atm = self.scenery.atmosphere();
        let snow = self.snow_color();
        let pack = self.snowpack();
        let wall = house.color.scale(0.35 + 0.5 * atm.daylight);
        let (x0, x1) = (house.x, house.x + house.width);
        let top = ground - house.wall;
        for y in top as i32..ground as i32 {
            for x in x0 as i32..x1 as i32 {
                canvas.solid(x, y, wall);
            }
        }
        // Techo a dos aguas, cubierto de nieve; la nieve se engrosa con la canción.
        let over = 1.5;
        let mid = (x0 + x1) / 2.0;
        let roof_top = top - house.roof;
        let thick = 1.0 + 2.0 * pack;
        for dy in 0..house.roof.ceil() as i32 + 1 {
            let k = dy as f32 / house.roof;
            let half = k * (house.width / 2.0 + over);
            for dx in -(half as i32)..=half as i32 {
                let from_edge = half - dx.abs() as f32;
                let c = if from_edge < thick || dy as f32 >= house.roof - 0.5 { snow } else { Rgb(0.30, 0.10, 0.08).scale(0.4 + 0.4 * atm.daylight) };
                canvas.solid((mid + dx as f32) as i32, (roof_top + dy as f32) as i32, c);
            }
        }
        if house.chimney {
            let cx = mid + house.width * 0.22;
            let cy = roof_top + house.roof * 0.45;
            for y in (cy - house.roof * 0.45) as i32..cy as i32 {
                canvas.solid(cx as i32, y, Rgb(0.32, 0.18, 0.14).scale(0.5 + 0.4 * atm.daylight));
                canvas.solid(cx as i32 + 1, y, Rgb(0.28, 0.15, 0.12).scale(0.5 + 0.4 * atm.daylight));
            }
            canvas.solid(cx as i32, (cy - house.roof * 0.45) as i32 - 1, snow);
            canvas.solid(cx as i32 + 1, (cy - house.roof * 0.45) as i32 - 1, snow);
        }
        // Ventanas.
        let woken = self.woken.iter().find(|w| w.0 == lot).map(|w| (1.0 - w.1 / 12.0).max(0.0)).unwrap_or(0.0);
        let glow = self.bands.level.min(1.0);
        let lit_color = self.window_color(pal, glow);
        let dark = Rgb(0.08, 0.09, 0.14).mix(self.scenery.sky(0.3), 0.2);
        let gap = house.width / house.cols as f32;
        for row in 0..house.rows {
            for col in 0..house.cols {
                let window = row * house.cols + col;
                let size = if gap >= 7.0 && house.wall >= 9.0 { 3 } else { 2 };
                let wx = x0 + gap * (col as f32 + 0.5) - size as f32 / 2.0;
                let wy = top + 2.0 + row as f32 * (size as f32 + 3.0);
                if wy + size as f32 > ground - 1.0 {
                    continue;
                }
                let twinkle = self.twinkles.iter().filter(|t| t.lot == lot && t.window % (house.rows * house.cols) == window).map(|t| 1.0 - t.age / 0.6).fold(0.0, f32::max);
                let on = self.lit(lot, window);
                let c = if on { lit_color.add(Rgb::WHITE, 0.35 * twinkle + 0.2 * woken) } else { dark.add(lit_color, 0.6 * twinkle) };
                for dy in 0..size {
                    for dx in 0..size {
                        canvas.solid(wx as i32 + dx, wy as i32 + dy, c);
                    }
                }
                if on && atm.daylight < 0.6 {
                    canvas.glow(wx as i32 + size / 2, wy as i32 + size / 2, lit_color, 0.12 * (1.0 - atm.daylight) + 0.2 * woken);
                }
            }
        }
        // Nieve amontonada contra la pared.
        let drift = (1.0 + 2.0 * pack) as i32;
        for x in (x0 - 1.0) as i32..(x1 + 1.0) as i32 {
            let bump = ((x as f32 * 0.7).sin() * 0.5 + 0.5) * drift as f32;
            for y in (ground - bump) as i32..ground as i32 + 1 {
                canvas.solid(x, y, snow);
            }
        }
    }

    fn draw_church(&self, canvas: &mut Canvas, pal: &Palette, cx: f32, ground: f32, h: f32) {
        let atm = self.scenery.atmosphere();
        let snow = self.snow_color();
        let stone = Rgb(0.45, 0.43, 0.48).scale(0.35 + 0.5 * atm.daylight);
        let lw = Self::lot_width(h);
        let (bw, bh) = (lw * 0.8, h * 0.12);
        let (tw, th) = (lw * 0.32, h * 0.13);
        for y in (ground - bh) as i32..ground as i32 {
            for x in (cx - bw / 2.0) as i32..(cx + bw / 2.0) as i32 {
                canvas.solid(x, y, stone);
            }
        }
        // Torre con campanario y aguja.
        let tower_top = ground - bh - th;
        for y in tower_top as i32..(ground - bh) as i32 + 1 {
            for x in (cx - tw / 2.0) as i32..(cx + tw / 2.0) as i32 {
                canvas.solid(x, y, stone.scale(1.08));
            }
        }
        let spire = tw * 1.4;
        for dy in 0..spire as i32 {
            let half = dy as f32 / spire * (tw / 2.0 + 1.0);
            for dx in -(half as i32)..=half as i32 {
                let c = if half - dx.abs() as f32 <= 1.0 { snow } else { Rgb(0.20, 0.25, 0.30).scale(0.5 + 0.4 * atm.daylight) };
                canvas.solid((cx + dx as f32) as i32, (tower_top - spire + dy as f32) as i32, c);
            }
        }
        // El campanario brilla cuando suena la campana.
        let bell_y = tower_top + th * 0.3;
        let warm = self.window_color(pal, 1.0);
        let bell_c = Rgb(0.10, 0.10, 0.14).mix(warm, 0.35 + 0.65 * self.bell);
        for dy in 0..3 {
            canvas.solid(cx as i32 - 1, bell_y as i32 + dy, bell_c);
            canvas.solid(cx as i32, bell_y as i32 + dy, bell_c);
        }
        if self.bell > 0.02 {
            canvas.glow(cx as i32, bell_y as i32 + 1, warm, 0.5 * self.bell);
            // Ondas de la campana que se abren.
            for ring in 0..2 {
                let age = (1.0 - self.bell) + ring as f32 * 0.25;
                let r = age * h * 0.35;
                let alpha = (0.4 * self.bell * (1.0 - age)).max(0.0);
                if alpha <= 0.0 || r < 2.0 {
                    continue;
                }
                let steps = (r * 6.0) as i32;
                for s in 0..steps {
                    let a = s as f32 / steps as f32 * std::f32::consts::TAU;
                    canvas.paint((cx + a.cos() * r) as i32, (bell_y + a.sin() * r * 0.6) as i32, warm.add(Rgb::WHITE, 0.3), alpha);
                }
            }
        }
        // Puerta y vitral.
        let door = self.window_color(pal, 0.5).scale(0.5 + 0.5 * self.progress);
        for y in (ground - 5.0) as i32..ground as i32 {
            canvas.solid(cx as i32 - 1, y, door);
            canvas.solid(cx as i32, y, door);
        }
        let rose = pal.petal.mix(warm, 0.5).scale(0.5 + 0.5 * self.progress.max(self.bell));
        canvas.solid(cx as i32 - (bw * 0.3) as i32, (ground - bh * 0.6) as i32, rose);
        canvas.solid(cx as i32 + (bw * 0.3) as i32, (ground - bh * 0.6) as i32, rose);
        // Techo de la nave, nevado.
        for x in (cx - bw / 2.0 - 1.0) as i32..(cx + bw / 2.0 + 1.0) as i32 {
            canvas.solid(x, (ground - bh) as i32 - 1, snow);
        }
    }

    /// Humo de las chimeneas, que se va con el viento.
    fn draw_smoke(&self, canvas: &mut Canvas, houses: &[(usize, House)], ground: f32, h: f32) {
        let wind = self.scenery.wind();
        let smoke = Rgb(0.70, 0.72, 0.78).scale(0.5 + 0.4 * self.scenery.atmosphere().daylight);
        for (lot, house) in houses {
            if !house.chimney {
                continue;
            }
            let cx = house.x + house.width / 2.0 + house.width * 0.22;
            let cy = ground - house.wall - house.roof * 1.0;
            for k in 0..7 {
                let age = (self.time * 0.25 + k as f32 / 7.0 + hash(*lot as u32, 9)) % 1.0;
                let x = cx + age * h * (0.10 + 0.35 * wind) + (age * 7.0 + self.time * 0.8).sin() * 1.5;
                let y = cy - age * h * 0.16;
                let r = 1.0 + age * 3.0;
                let alpha = 0.3 * (1.0 - age);
                for dy in -(r as i32)..=r as i32 {
                    for dx in -(r as i32)..=r as i32 {
                        if (dx * dx + dy * dy) as f32 <= r * r {
                            canvas.paint(x as i32 + dx, y as i32 + dy, smoke, alpha);
                        }
                    }
                }
            }
        }
    }

    /// Faroles de la calle, con su halo.
    fn draw_lamps(&self, canvas: &mut Canvas, w: f32, h: f32) -> [(f32, f32); 2] {
        let warm = Rgb(1.0, 0.82, 0.50);
        let ground = h * GROUND;
        let mut spots = [(0.0, 0.0); 2];
        for (i, fx) in [0.18, 0.78].into_iter().enumerate() {
            let x = (w * fx) as i32;
            let top = ground - h * 0.12;
            for y in top as i32..(ground + h * 0.06) as i32 {
                canvas.solid(x, y, Rgb(0.08, 0.08, 0.10));
            }
            canvas.solid(x - 1, top as i32, warm);
            canvas.solid(x, top as i32, warm);
            canvas.solid(x + 1, top as i32, warm);
            canvas.glow(x, top as i32, warm, 0.45 + 0.15 * self.bands.level.min(1.0));
            spots[i] = (x as f32, top);
        }
        spots
    }

    /// Nieve que cae en tres capas, con vaivén y viento. Cerca de un farol brilla.
    fn draw_snowfall(&self, canvas: &mut Canvas, lamps: &[(f32, f32); 2]) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let atm = self.scenery.atmosphere();
        let amount = 0.002 + 0.005 * self.bands.intensity.min(1.0) + 0.004 * atm.rain;
        let flakes = (w * h * amount) as u32;
        let wind = self.scenery.wind();
        let t = self.time;
        let base = Rgb(0.90, 0.93, 1.0).scale(0.55 + 0.45 * atm.daylight);
        for i in 0..flakes {
            let layer = (hash(i, 7) * 3.0) as u32;
            let speed = [0.05, 0.08, 0.12][layer as usize] * (1.0 + 0.3 * hash(i, 2));
            let y = ((hash(i, 3) + t * speed) % 1.0) * h;
            let sway = (t * (0.6 + hash(i, 4)) + i as f32).sin() * (1.0 + layer as f32);
            let x = (hash(i, 1) * w + sway + t * wind * 6.0 * (1.0 + layer as f32)).rem_euclid(w);
            let near_lamp = lamps.iter().map(|&(lx, ly)| ((x - lx).powi(2) + ((y - ly) * 1.5).powi(2)).sqrt()).fold(f32::MAX, f32::min);
            let lit = (1.0 - near_lamp / (h * 0.15)).max(0.0);
            let c = base.mix(Rgb(1.0, 0.85, 0.55), lit);
            let alpha = [0.35, 0.55, 0.85][layer as usize];
            canvas.paint(x as i32, y as i32, c, alpha);
            if layer == 2 {
                canvas.paint(x as i32 + 1, y as i32, c, alpha * 0.6);
            }
        }
        for p in &self.poof {
            let a = (1.0 - p.age / 1.5) * 0.9;
            canvas.paint((p.x * w) as i32, (p.y * h) as i32, base.add(Rgb::WHITE, 0.3), a);
        }
    }
}

impl Scene for Village {
    fn update(&mut self, frame: &Frame, w: usize, h: usize) {
        self.advance(frame, w, h);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        if w < 2.0 || h < 2.0 {
            return;
        }
        let ground = (h * GROUND).floor();
        let view = View { cx: w / 2.0, ground, scale: ground.max(4.0), weight: 1.0 };
        let scenery = &self.scenery;
        canvas.clear(|_, y| scenery.sky((y as f32 / ground).min(1.0)).mix(pal.bg_bottom, 0.06));
        self.scenery.draw_back(canvas, &view, pal);
        self.draw_mountains(canvas);
        // El pueblo, solar por solar.
        let lots = Self::lots(w, h);
        let lw = Self::lot_width(h);
        let mut houses = Vec::new();
        for i in 0..lots {
            match self.lot(i, lots) {
                Lot::House => houses.push((i, self.house(i, h))),
                Lot::Pine => {
                    let size = h * (0.13 + 0.06 * hash(self.seed ^ 0xB2, i as u32));
                    self.draw_pine(canvas, i as f32 * lw + lw / 2.0, ground + 1.0, size);
                }
                Lot::Church => self.draw_church(canvas, pal, i as f32 * lw + lw / 2.0, ground, h),
            }
        }
        for (i, house) in &houses {
            self.draw_house(canvas, pal, *i, house, ground);
        }
        self.draw_smoke(canvas, &houses, ground, h);
        // Suelo nevado, con destellos que titilan despacio.
        let snow = self.snow_color();
        let slow = (self.time * 0.7) as u32;
        for y in ground as i32..h as i32 {
            let v = (y as f32 - ground) / (h - ground).max(1.0);
            for x in 0..canvas.w as i32 {
                let mut c = snow.scale(0.92 + 0.08 * v).mix(self.scenery.sky(0.1), 0.06 * (1.0 - v));
                if hash(x as u32 ^ self.seed, y as u32 ^ slow.wrapping_mul(7919)) < 0.006 {
                    c = c.add(Rgb::WHITE, 0.5);
                }
                canvas.solid(x, y, c);
            }
        }
        // Huella de un camino que cruza el pueblo.
        for x in 0..canvas.w as i32 {
            let py = ground + (h - ground) * (0.45 + 0.12 * (x as f32 / h * 2.0).sin());
            canvas.solid(x, py as i32, snow.scale(0.82));
        }
        // Dos pinos grandes adelante, en las orillas, enmarcan el pueblo.
        self.draw_pine(canvas, w * 0.03, h + 2.0, h * 0.42);
        self.draw_pine(canvas, w * 0.97, h + 2.0, h * 0.36);
        let lamps = self.draw_lamps(canvas, w, h);
        self.draw_snowfall(canvas, &lamps);
    }

    fn new_track(&mut self) {
        // Pueblo nuevo para la canción nueva: lo tapa la transición.
        self.seed = (self.rng.next() * 1e6) as u32;
        self.twinkles.clear();
        self.woken.clear();
        self.bell = 0.0;
        self.scenery.fade();
    }

    /// Un click enciende la casa de ese solar y levanta un remolino de nieve.
    fn poke(&mut self, x: f32, y: f32) {
        let lot = (x * self.size.0 / Self::lot_width(self.size.1)) as usize;
        self.woken.retain(|w| w.0 != lot);
        self.woken.push((lot, 0.0));
        for _ in 0..40 {
            if self.poof.len() >= MAX_POOF {
                break;
            }
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            let s = self.rng.range(0.05, 0.25);
            self.poof.push(Flake { x, y, vx: a.cos() * s, vy: a.sin() * s - 0.08, age: 0.0 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(v: &mut Village, bands: &Bands, progress: f32, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            v.update(&Frame { dt: 1.0 / 60.0, bands, progress: Some(progress), song_len: 180.0, playing: true }, 120, 34);
        }
    }

    fn lit_share(v: &Village) -> f32 {
        let on = (0..20).flat_map(|l| (0..8).map(move |k| (l, k))).filter(|&(l, k)| v.lit(l, k)).count();
        on as f32 / 160.0
    }

    #[test]
    fn the_village_lights_up_as_the_song_goes_on() {
        let mut v = Village::new(1);
        run(&mut v, &Bands::default(), 0.05, 0.1);
        let early = lit_share(&v);
        run(&mut v, &Bands::default(), 0.95, 0.1);
        let late = lit_share(&v);
        assert!(early < 0.35 && late > 0.7, "{early} {late}");
        assert!(v.snowpack() > 0.9);
    }

    #[test]
    fn the_climax_rings_the_bell_and_beats_twinkle() {
        let mut v = Village::new(2);
        run(&mut v, &Bands { climax: true, beat: true, ..Bands::default() }, 0.5, 1.0 / 60.0);
        assert!(v.bell > 0.99);
        assert_eq!(v.twinkles.len(), 1);
        run(&mut v, &Bands::default(), 0.5, 10.0);
        assert!(v.bell < 0.05 && v.twinkles.is_empty());
    }

    #[test]
    fn there_is_always_one_church_in_the_middle() {
        let v = Village::new(3);
        for lots in [2, 5, 13, 40] {
            let churches = (0..lots).filter(|&i| v.lot(i, lots) == Lot::Church).count();
            assert_eq!(churches, 1);
        }
    }

    #[test]
    fn a_click_wakes_that_house() {
        let mut v = Village::new(4);
        run(&mut v, &Bands::default(), 0.0, 0.1);
        let lot = (0.5 * v.size.0 / Village::lot_width(v.size.1)) as usize;
        v.poke(0.5, 0.6);
        assert!((0..8).all(|k| v.lit(lot, k)));
        assert!(!v.poof.is_empty());
    }

    #[test]
    fn draws_at_any_size() {
        let mut v = Village::new(5);
        let bands = Bands { level: 1.0, intensity: 1.0, beat: true, climax: true, ..Bands::default() };
        run(&mut v, &bands, 0.7, 1.0);
        v.poke(0.3, 0.3);
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (220, 60)] {
            let mut canvas = Canvas::new(w, h);
            v.draw(&mut canvas, &Palette::default());
        }
    }
}

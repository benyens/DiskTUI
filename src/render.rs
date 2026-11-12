//! Dibuja la planta en el lienzo: fondo, resplandor, tallo, hojas, flor, tierra y partículas.

use std::f32::consts::PI;

use crate::canvas::Canvas;
use crate::garden::{Garden, Member};
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::{Flower, LEAF_LENGTH, ParticleKind, Plant, smoothstep};

/// Filas de celdas reservadas para la tierra.
pub const SOIL_ROWS: usize = 2;

/// Transformación de coordenadas de mundo a píxeles del lienzo.
#[derive(Clone, Copy)]
pub struct View {
    pub cx: f32,
    /// Fila de píxel donde está el suelo.
    pub ground: f32,
    /// Píxeles por unidad de mundo.
    pub scale: f32,
    /// Grosor relativo del trazo: 1 para la planta principal, menos para las laterales.
    pub weight: f32,
}

impl View {
    /// Vista de la planta principal. `half_span`: medio ancho que debe caber, en unidades
    /// de mundo.
    pub fn new(canvas: &Canvas, half_span: f32) -> Self {
        Self::for_size(canvas.w, canvas.h, half_span)
    }

    /// Igual que [`View::new`], a partir del tamaño del lienzo en celdas.
    pub fn for_size(w: usize, h: usize, half_span: f32) -> Self {
        let ground = (h.saturating_sub(SOIL_ROWS) * 2) as f32;
        // Limitada por alto y por ancho, para que ninguna flor se corte.
        let scale = (ground * 0.97).min(w as f32 / (2.0 * half_span)).max(4.0);
        View { cx: w as f32 / 2.0, ground, scale, weight: 1.0 }
    }

    /// Vista de una planta del jardín, desplazada y escalada respecto a la principal.
    pub fn member(&self, m: &Member) -> View {
        View {
            cx: self.cx + m.x * self.scale,
            ground: self.ground,
            scale: self.scale * m.scale,
            weight: m.scale,
        }
    }

    pub fn px(&self, x: f32, y: f32) -> (f32, f32) {
        (self.cx + x * self.scale, self.ground - y * self.scale)
    }

    /// Medio ancho visible, en unidades de mundo.
    pub fn half_width(&self) -> f32 {
        self.cx / self.scale
    }
}

pub fn draw(canvas: &mut Canvas, garden: &Garden, pal: &Palette) {
    let main_view = View::new(canvas, garden.half_span());
    let ph = canvas.px_h().max(1) as f32;
    // El fondo se tiñe con la intensidad de la canción y destella en cada clímax.
    let main = garden.main();
    let lift = 0.022 * main.bands.intensity + 0.09 * main.climax_pulse;
    // El cielo lo pone el clima; la carátula lo tiñe apenas.
    let scenery = &garden.scenery;
    canvas.clear(|_, y| scenery.sky(y as f32 / ph).mix(pal.bg_bottom, 0.12).add(pal.petal, lift));

    // Cielo y cerros, detrás de todo.
    garden.scenery.draw_back(canvas, &main_view, pal);

    // Las laterales primero: quedan detrás de la principal.
    let layers: Vec<(&Plant, View, Palette, Flower)> = garden
        .members
        .iter()
        .rev()
        .map(|m| (&m.plant, main_view.member(m), m.palette(pal), m.plant.flower()))
        .collect();

    // De día los resplandores casi no se ven.
    let dusk = 1.0 - 0.7 * garden.scenery.atmosphere().daylight;
    for (plant, view, colors, flower) in &layers {
        draw_glow(canvas, view, plant, flower, colors, dusk);
    }
    for (plant, view, colors, _) in &layers {
        draw_tendrils(canvas, view, plant, colors);
        draw_stem(canvas, view, plant, colors);
        draw_leaves(canvas, view, plant, colors);
    }
    for (plant, view, colors, flower) in &layers {
        draw_flower(canvas, view, plant, flower, colors);
    }
    draw_grass(canvas, &main_view, garden.main(), pal);
    let stems: Vec<f32> = garden.members.iter().map(|m| m.x).collect();
    garden.scenery.draw_ground(canvas, &main_view, pal, &stems);
    draw_soil(canvas, &main_view, garden.main(), pal);
    for (plant, view, colors, _) in &layers {
        draw_particles(canvas, view, plant, colors);
    }
    garden.scenery.draw_front(canvas, &main_view, pal);
}

/// Halo de luz detrás de la flor y destello en la base con cada bombo.
fn draw_glow(canvas: &mut Canvas, view: &View, plant: &Plant, f: &Flower, pal: &Palette, dusk: f32) {
    let b = &plant.bands;
    let presence = smoothstep(0.0, 0.3, plant.growth) * (0.4 + 0.6 * f.open);
    let intensity = (0.05 + 0.26 * b.level + 0.16 * plant.hat_pulse + 0.08 * plant.kick_pulse)
        * (0.7 + 0.7 * b.intensity + 0.9 * plant.climax_pulse)
        * presence
        * view.weight
        * dusk;
    let (fx, fy) = view.px(f.x, f.y);
    let r = (f.radius * view.scale).max(1.0);
    disc(canvas, fx, fy, r * 3.2, |d| intensity * (-(d / (r * 1.5)).powi(2)).exp(), pal.petal);

    let base = 0.22 * plant.kick_pulse * smoothstep(0.0, 0.05, plant.growth);
    if base > 0.01 {
        let (bx, by) = view.px(0.0, 0.0);
        let br = 0.09 * view.scale;
        disc(canvas, bx, by, br * 2.5, |d| base * (-(d / br).powi(2)).exp(), pal.accent);
    }
}

/// Suma luz en un disco. `falloff(distancia)` da la intensidad.
fn disc(canvas: &mut Canvas, cx: f32, cy: f32, radius: f32, falloff: impl Fn(f32) -> f32, color: Rgb) {
    let (x0, x1) = ((cx - radius).floor() as i32, (cx + radius).ceil() as i32);
    let (y0, y1) = ((cy - radius).floor() as i32, (cy + radius).ceil() as i32);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let k = falloff(d);
            if k > 0.004 {
                canvas.glow(x, y, color, k);
            }
        }
    }
}

/// Brillo que aportan los pulsos de luz en la posición `s` del tallo.
fn pulse_light(plant: &Plant, s: f32) -> f32 {
    plant
        .pulses
        .iter()
        .map(|p| (-((s - p) / 0.035).powi(2)).exp())
        .sum::<f32>()
        .min(1.0)
}

fn draw_stem(canvas: &mut Canvas, view: &View, plant: &Plant, pal: &Palette) {
    let g = plant.growth;
    if g < 0.002 {
        return;
    }
    let bass = plant.bands.bass;
    let n = ((g * 320.0).ceil() as usize).max(2);
    let mut prev: Option<(f32, f32)> = None;
    for i in 0..=n {
        let s = g * i as f32 / n as f32;
        let (wx, wy) = plant.stem_point(s);
        let (px, py) = view.px(wx, wy);
        let (dx, dy) = (px * 2.0, py * 2.0);
        if let Some((qx, qy)) = prev {
            let color = pal
                .stem
                .mix(pal.leaf, 0.6 * s)
                .add(Rgb::WHITE, 0.06 * bass)
                .mix(pal.accent, 0.85 * pulse_light(plant, s));
            // Grueso en la base, fino en la punta; late con los graves.
            let taper = (1.0 - s / g.max(0.05)).max(0.0).powf(0.7);
            let thick = 1.0 + (1.9 * taper * smoothstep(0.0, 0.25, g) + 0.9 * bass * (1.0 - s)) * view.weight;
            let lines = (thick.round() as i32).clamp(1, 4);
            for k in 0..lines {
                let off = k as f32 - (lines - 1) as f32 / 2.0;
                canvas.line(qx + off, qy, dx + off, dy, color);
            }
        }
        prev = Some((dx, dy));
    }
}

fn draw_leaves(canvas: &mut Canvas, view: &View, plant: &Plant, pal: &Palette) {
    let bass = plant.bands.bass;
    for leaf in &plant.leaves {
        let (bx, by) = plant.stem_point(leaf.s);
        let t = (leaf.age / 1.4).clamp(0.0, 1.0);
        let grow = 1.0 - (1.0 - t).powi(3);
        let len = LEAF_LENGTH * leaf.size * grow * (1.0 + 0.10 * bass + 0.06 * plant.kick_pulse);
        if len * view.scale < 0.6 {
            continue;
        }
        let angle = leaf.angle + 0.10 * (plant.time * 1.4 + leaf.s * 9.0).sin() * (0.3 + bass);
        let (dx, dy) = (leaf.side * angle.cos(), angle.sin());
        let base = pal.leaf.mix(pal.stem, 0.5 * leaf.tint);
        let light = pulse_light(plant, leaf.s);

        let len_dots = len * view.scale * 2.0;
        let steps = (len_dots * 1.6).ceil().max(2.0) as usize;
        for i in 0..=steps {
            let u = i as f32 / steps as f32;
            // Nervio central con una leve caída hacia la punta.
            let wx = bx + dx * len * u;
            let wy = by + dy * len * u - 0.30 * len * u * u;
            let (px, py) = view.px(wx, wy);
            let half = 0.15 * len * (PI * u.powf(0.62)).sin().max(0.0).powf(1.25) * view.scale * 2.0;
            let color = base.mix(Rgb::WHITE, 0.14 * u).mix(pal.accent, 0.5 * light);
            let m = half.ceil() as i32;
            for j in -m..=m {
                if (j as f32).abs() > half {
                    continue;
                }
                // Un hueco en el centro dibuja el nervio, si la hoja es bastante ancha.
                if j == 0 && half > 1.6 && u > 0.10 && u < 0.92 {
                    continue;
                }
                // Normal al eje de la hoja, en espacio de puntos (y invertida).
                let x = px * 2.0 + dy * j as f32;
                let y = py * 2.0 + dx * j as f32;
                canvas.dot(x.round() as i32, y.round() as i32, color);
            }
        }
    }
}

fn draw_tendrils(canvas: &mut Canvas, view: &View, plant: &Plant, pal: &Palette) {
    const STEPS: usize = 70;
    let color = pal.leaf.mix(pal.stem, 0.35);
    for t in &plant.tendrils {
        let (mut x, mut y) = plant.stem_point(t.s);
        let grow = (t.age / 2.2).clamp(0.0, 1.0);
        let total = 0.17 * t.size;
        let step = total / STEPS as f32;
        // Sale hacia afuera y arriba, y se enrosca cada vez más cerrado.
        let mut heading = if t.side > 0.0 { 0.9 } else { PI - 0.9 };
        heading += 0.15 * (plant.time * 1.2 + t.s * 11.0).sin() * plant.bands.bass;
        let mut prev = view.px(x, y);
        for i in 0..(STEPS as f32 * grow) as usize {
            let u = i as f32 / STEPS as f32;
            heading += t.side * (6.0 + 190.0 * u * u * u) * step;
            x += heading.cos() * step;
            y += heading.sin() * step;
            let cur = view.px(x, y);
            canvas.line(prev.0 * 2.0, prev.1 * 2.0, cur.0 * 2.0, cur.1 * 2.0, color);
            prev = cur;
        }
    }
}

/// Flor: rosa polar por capas, `r(θ) = |cos(n·θ/2)|^p`, con suavizado por supermuestreo.
pub fn draw_flower(canvas: &mut Canvas, view: &View, plant: &Plant, f: &Flower, pal: &Palette) {
    let (cx, cy) = view.px(f.x, f.y);
    let radius = f.radius * view.scale;
    if radius < 0.4 {
        return;
    }
    let shader = FlowerShader::new(plant, f, pal, radius);
    let reach = radius * 1.12 + 1.0;
    let (x0, x1) = ((cx - reach).floor() as i32, (cx + reach).ceil() as i32);
    let (y0, y1) = ((cy - reach).floor() as i32, (cy + reach).ceil() as i32);
    const SS: usize = 3;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (mut acc, mut hits) = ((0.0f32, 0.0f32, 0.0f32), 0u32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let dx = x as f32 + (sx as f32 + 0.5) / SS as f32 - cx;
                    let dy = cy - (y as f32 + (sy as f32 + 0.5) / SS as f32);
                    if let Some(c) = shader.sample(dx, dy) {
                        acc = (acc.0 + c.0, acc.1 + c.1, acc.2 + c.2);
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                let n = hits as f32;
                canvas.blend(x, y, Rgb(acc.0 / n, acc.1 / n, acc.2 / n), n / (SS * SS) as f32);
            }
        }
    }

    // Pistilos: semillas en espiral de Fibonacci. Los medios encienden más semillas.
    let seeds = (5.0 + 30.0 * plant.bands.mid * f.open) as usize;
    let seed_color = pal.accent.mix(Rgb::WHITE, 0.55);
    for i in 1..=seeds.min(34) {
        let r = 0.36 * radius * (i as f32 / 34.0).sqrt() * (1.0 + 0.2 * plant.bands.treble);
        let a = i as f32 * 2.399_963 - plant.rot * 1.7;
        let twinkle = 0.55 + 0.45 * (plant.time * 6.0 + i as f32).sin() * plant.bands.treble;
        canvas.blend(
            (cx + a.cos() * r).floor() as i32,
            (cy - a.sin() * r).floor() as i32,
            seed_color,
            (0.85 * twinkle * smoothstep(0.1, 0.5, f.open)).clamp(0.0, 1.0),
        );
    }
}

/// Una capa de pétalos.
struct Layer {
    scale: f32,
    rot: f32,
    inner: Rgb,
    outer: Rgb,
}

struct FlowerShader {
    radius: f32,
    petals: f32,
    /// Exponente de la rosa: alto = pétalos finos (cerrada), bajo = anchos (abierta).
    sharp: f32,
    /// Radio mínimo entre pétalos: alto = capullo redondo.
    core: f32,
    ruffle: f32,
    time: f32,
    layers: [Layer; 3],
    pistil: f32,
    pistil_color: Rgb,
}

impl FlowerShader {
    fn new(plant: &Plant, f: &Flower, pal: &Palette, radius: f32) -> Self {
        let open = f.open;
        let n = f.petals as f32;
        // El capullo nace verde y toma el color de los pétalos al abrirse.
        let bloom = smoothstep(0.0, 0.35, open);
        let petal = pal.leaf.mix(pal.petal, bloom);
        let inner_grow = smoothstep(0.15, 0.75, open);
        // Huella de la canción: pétalos puntiagudos si es menor, redondos si es mayor; dos
        // capas si es calmada, tres si es enérgica.
        let pointed = f.genes.map_or(0.0, |g| g.sharp);
        let third = if f.genes.is_some_and(|g| g.layers < 3) { 0.0 } else { 1.0 };
        FlowerShader {
            radius,
            petals: n,
            sharp: 2.4 + (0.6 + 0.55 * pointed - 2.4) * open,
            core: 0.78 + (0.16 - 0.06 * pointed - 0.78) * open,
            ruffle: 0.05 * plant.bands.treble,
            time: plant.time,
            layers: [
                Layer {
                    scale: 1.0,
                    rot: f.rot,
                    inner: petal.scale(0.55),
                    outer: petal.mix(Rgb::WHITE, 0.22),
                },
                Layer {
                    scale: 0.64 * inner_grow,
                    rot: -f.rot * 0.6 + PI / n,
                    inner: pal.petal.scale(0.8),
                    outer: pal.petal_light,
                },
                Layer {
                    scale: 0.37 * inner_grow * third,
                    rot: f.rot * 1.4,
                    inner: pal.petal_light.mix(pal.accent, 0.35),
                    outer: pal.petal_light.mix(pal.accent, 0.65),
                },
            ],
            pistil: (0.15 + 0.04 * plant.bands.level) * smoothstep(0.2, 0.6, open),
            pistil_color: pal.accent,
        }
    }

    /// Color en el punto (dx, dy) relativo al centro, `y` hacia arriba. `None` = fuera.
    fn sample(&self, dx: f32, dy: f32) -> Option<Rgb> {
        let r = (dx * dx + dy * dy).sqrt() / self.radius;
        if r > 1.12 {
            return None;
        }
        if r < self.pistil {
            let t = r / self.pistil;
            return Some(self.pistil_color.mix(Rgb::WHITE, 0.35 * (1.0 - t)).scale(1.0 - 0.3 * t * t));
        }
        let theta = dy.atan2(dx);
        let mut out = None;
        for layer in &self.layers {
            if layer.scale < 0.02 {
                continue;
            }
            let shape = (self.petals * (theta - layer.rot) / 2.0).cos().abs().powf(self.sharp);
            let wave = 1.0 + self.ruffle * (theta * self.petals * 3.0 + self.time * 9.0).sin();
            let edge = layer.scale * (self.core + (1.0 - self.core) * shape) * wave;
            if r < edge {
                let t = r / edge;
                // Degradado del centro a la punta; más claro sobre el eje del pétalo, más
                // oscuro en los valles, para separar un pétalo del vecino.
                let c = layer.inner.mix(layer.outer, smoothstep(0.0, 1.0, t));
                out = Some(c.scale(0.78 + 0.22 * shape));
            }
        }
        out
    }
}

/// Tierra con `░▒▓█`. Los graves la recorren en ondas desde la base del tallo.
fn draw_soil(canvas: &mut Canvas, view: &View, plant: &Plant, pal: &Palette) {
    const SHADES: [char; 5] = [' ', '░', '▒', '▓', '█'];
    let b = &plant.bands;
    // Fondo oscuro bajo la tierra: sin esto, de día se transparenta el cielo claro.
    let under = pal.soil.scale(0.45);
    for y in (canvas.h.saturating_sub(SOIL_ROWS) * 2)..canvas.px_h() {
        for x in 0..canvas.w {
            canvas.paint(x as i32, y as i32, under, 1.0);
        }
    }
    for row in 0..SOIL_ROWS.min(canvas.h) {
        let cy = canvas.h - SOIL_ROWS.min(canvas.h) + row;
        for cx in 0..canvas.w {
            let noise = hash(cx as u32, row as u32);
            let dist = (cx as f32 + 0.5 - view.cx).abs();
            let ripple = b.bass
                * (-dist / (canvas.w as f32 * 0.22).max(1.0)).exp()
                * (0.5 + 0.5 * (dist * 0.55 - plant.time * 7.0).sin())
                + 0.8 * plant.kick_pulse * (-dist / 6.0).exp();
            let level = (1.0 + row as f32 + noise * 1.2 + ripple * 1.7) as usize;
            let color = pal
                .soil
                .scale(0.75 + 0.5 * noise)
                .mix(pal.accent, (0.38 * ripple).min(0.6));
            canvas.glyph(cx as i32, cy as i32, SHADES[level.clamp(1, 4)], color, 1.0);
        }
    }
}

/// Briznas de pasto sobre la línea del suelo. Se estiran un poco con los graves.
fn draw_grass(canvas: &mut Canvas, view: &View, plant: &Plant, pal: &Palette) {
    let ground = (view.ground * 2.0) as i32; // en puntos Braille
    let color = pal.stem.mix(pal.leaf, 0.35);
    for x in 0..(canvas.w * 2) as u32 {
        let n = hash(x, 77);
        if n > 0.34 {
            continue;
        }
        let dist = (x as f32 / 2.0 - view.cx).abs();
        let wave = plant.bands.bass * (-dist / (canvas.w as f32 * 0.3).max(1.0)).exp();
        let height = 1.0 + hash(x, 78) * 2.6 + wave * 2.0;
        let lean = ((plant.time * 1.6 + x as f32 * 0.35).sin() * (0.4 + plant.bands.bass)).round() as i32;
        for k in 0..height.round() as i32 {
            let bend = if k >= 2 { lean } else { 0 };
            canvas.dot(x as i32 + bend, ground - 1 - k, color.mix(pal.leaf, hash(x, 79) * 0.6));
        }
    }
}

/// Ruido determinista 0..1 por celda.
fn hash(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77).rotate_left(13);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

fn draw_particles(canvas: &mut Canvas, view: &View, plant: &Plant, pal: &Palette) {
    let max_row = canvas.h.saturating_sub(SOIL_ROWS) as i32;
    // Orden: lo menos importante primero, para que lo demás lo tape.
    for kind in [ParticleKind::Mote, ParticleKind::Pollen, ParticleKind::Spark, ParticleKind::Petal] {
        for p in plant.particles.iter().filter(|p| p.kind == kind) {
            let (px, py) = view.px(p.x, p.y);
            let (cx, cy) = (px.floor() as i32, (py / 2.0).floor() as i32);
            if cy >= max_row {
                continue;
            }
            let t = p.age / p.life;
            let fade = smoothstep(0.0, 0.12, t) * (1.0 - smoothstep(0.55, 1.0, t));
            let (ch, color, alpha) = match kind {
                ParticleKind::Pollen => {
                    let ch = if t < 0.35 { '•' } else if t < 0.7 { '∙' } else { '·' };
                    (ch, pal.accent.mix(Rgb::WHITE, 0.25), fade)
                }
                ParticleKind::Spark => {
                    let ch = if t > 0.6 { '·' } else { ['✦', '✧', '⁕'][p.variant as usize % 3] };
                    (ch, pal.accent.mix(Rgb::WHITE, 0.5), fade)
                }
                ParticleKind::Petal => (
                    ['❀', '✿', '❁'][p.variant as usize % 3],
                    pal.petal.mix(Rgb::WHITE, 0.15),
                    fade * 0.95,
                ),
                ParticleKind::Mote => {
                    let twinkle = 0.22 + 0.35 * plant.bands.level + 0.18 * (p.phase + plant.time * 1.7).sin();
                    ('·', pal.petal_light, fade * twinkle.clamp(0.0, 1.0))
                }
            };
            canvas.glyph(cx, cy, ch, color, alpha);
        }
    }
}

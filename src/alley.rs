//! Escena "callejón": un callejón japonés de noche, con lluvia.
//!
//! Se ve en perspectiva, de un extremo al otro: paredes con ventanas y letreros de neón,
//! faroles de papel colgando, una máquina expendedora encendida, un gato sobre un cajón y,
//! al fondo, la luz de la calle. El suelo mojado refleja todo y cada gota hace una onda. Los
//! letreros titilan con los agudos y laten con el pulso; algunos están medio rotos. El
//! clímax los enciende a todos de golpe; entre canciones, se corta la luz un momento.

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// Punto de fuga, como fracción del lienzo.
const VANISH: (f32, f32) = (0.5, 0.42);
/// Distancia focal, como fracción del alto: más chica, más gran angular.
const FOCAL: f32 = 0.55;
/// Medio ancho del callejón, altura de los ojos sobre el suelo y profundidad del fondo.
const HALF_WIDTH: f32 = 1.0;
const EYE: f32 = 0.75;
const FAR: f32 = 14.0;
/// Largo de cada edificio, en profundidad.
const BUILDING: f32 = 3.0;
const MAX_RIPPLES: usize = 40;
const MAX_STEAM: usize = 30;

/// Un letrero: profundidad y altura de su rectángulo, color, cuánto brilla (con titileo)
/// y cuánta luz tira alrededor (sin titileo, para que las paredes no parpadeen enteras).
type Sign = (f32, f32, f32, f32, Rgb, f32, f32);

/// Tramos de edificio que se ven, por lado.
const SEGS: usize = (FAR / BUILDING) as usize + 2;

/// Qué se ve en un píxel.
enum Hit {
    Sky,
    /// Pared: lado (−1 izquierda, 1 derecha), profundidad y altura (negativa hacia arriba).
    Wall(f32, f32, f32),
    /// Suelo: x lateral y profundidad.
    Floor(f32, f32),
    /// La calle del fondo.
    End(f32, f32),
}

struct Ripple {
    x: f32,
    z: f32,
    age: f32,
}

struct Puff {
    x: f32,
    y: f32,
    age: f32,
}

pub struct Alley {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    progress: f32,
    bands: Bands,
    beat: f32,
    hat: f32,
    climax: f32,
    ripples: Vec<Ripple>,
    steam: Vec<Puff>,
    /// Corte de luz entre canciones.
    outage: Option<f32>,
    seed: u32,
}

fn hash(a: u32, b: u32) -> f32 {
    let h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    let h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
    (h ^ (h >> 13)) as f32 / u32::MAX as f32
}

impl Alley {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0xA11E_7001);
        scenery.bare = true;
        let mut rng = Rng::new(seed ^ 0x0A11_E700);
        let seed = (rng.next() * 1e6) as u32;
        Alley {
            scenery,
            rng,
            time: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            beat: 0.0,
            hat: 0.0,
            climax: 0.0,
            ripples: Vec::new(),
            steam: Vec::new(),
            outage: None,
            seed,
        }
    }

    /// Cuánta luz tienen los letreros: se corta entre canciones y vuelve parpadeando.
    fn power(&self) -> f32 {
        match self.outage {
            Some(t) if t < 0.35 => 1.0 - smoothstep(0.0, 0.15, t),
            Some(t) => {
                // Vuelve a tirones, como un tubo fluorescente.
                let flick = if (t * 37.0).sin() > 0.2 { 1.0 } else { 0.25 };
                smoothstep(0.35, 0.9, t) * flick
            }
            None => 1.0,
        }
    }

    /// Altura de los edificios (negativa: hacia arriba) del lado y tramo dados.
    fn roof(&self, side: f32, z: f32) -> f32 {
        let seg = (z / BUILDING).floor() as i32;
        -1.6 - 1.4 * hash(self.seed ^ if side < 0.0 { 11 } else { 23 }, seg as u32)
    }

    /// Lanza un rayo por el píxel (`dx`, `dy`) relativo al punto de fuga y escalado.
    fn trace(&self, dx: f32, dy: f32) -> Hit {
        let floor_z = if dy > 0.0 { EYE / dy } else { f32::INFINITY };
        let (side, wall_z) = if dx < 0.0 { (-1.0, HALF_WIDTH / -dx) } else if dx > 0.0 { (1.0, HALF_WIDTH / dx) } else { (1.0, f32::INFINITY) };
        let wall_y = dy * wall_z;
        let wall_ok = wall_z < FAR && wall_y >= self.roof(side, wall_z);
        if floor_z < FAR && (floor_z <= wall_z || !wall_ok) {
            return Hit::Floor(dx * floor_z, floor_z);
        }
        if wall_ok {
            return Hit::Wall(side, wall_z, wall_y);
        }
        let end_y = dy * FAR;
        if wall_z >= FAR && end_y >= -2.0 {
            return Hit::End(dx * FAR, end_y);
        }
        Hit::Sky
    }

    /// El letrero de neón de un edificio y lado, si tiene: su rectángulo (profundidad y
    /// altura), su color y cuánto brilla ahora (titila, late y algunos fallan).
    fn sign(&self, side: f32, seg: i32) -> Option<Sign> {
        if seg < 0 {
            return None;
        }
        let id = (seg * 2 + if side < 0.0 { 0 } else { 1 }) as u32 ^ self.seed;
        if hash(id, 1) < 0.25 {
            return None;
        }
        let z0 = seg as f32 * BUILDING + 0.4 + hash(id, 2) * 1.2;
        let (z1, y0, y1) = (z0 + 0.45, -1.5 + hash(id, 3) * 0.3, -0.35);
        let colors = [Rgb(1.0, 0.25, 0.55), Rgb(0.30, 0.85, 1.0), Rgb(1.0, 0.70, 0.25), Rgb(0.6, 0.4, 1.0), Rgb(0.35, 1.0, 0.55), Rgb(1.0, 0.35, 0.25)];
        let color = colors[(hash(id, 4) * 6.0) as usize % 6];
        // Un tercio de los letreros falla de vez en cuando.
        let broken = hash(id, 5) < 0.33 && (self.time * 3.0 + hash(id, 6) * 10.0).sin() > 0.75 && hash(id, (self.time * 9.0) as u32) < 0.6;
        let flicker = 0.8 + 0.2 * (self.time * (5.0 + hash(id, 7) * 4.0)).sin() * self.hat;
        let level = if broken { 0.15 } else { flicker * (0.75 + 0.35 * self.beat).min(1.0) }.max(self.climax);
        let glow = (0.8f32 * if broken { 0.3 } else { 1.0 }).max(self.climax) * self.power();
        Some((z0, z1, y0, y1, color, level * self.power(), glow))
    }

    /// El letrero en ese punto de la pared, con trazos como de letras.
    /// Los letreros de este cuadro, calculados una vez: `[lado izquierdo, lado derecho]`.
    fn signs(&self) -> [[Option<Sign>; SEGS]; 2] {
        let mut all = [[None; SEGS]; 2];
        for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
            for (seg, slot) in all[i].iter_mut().enumerate() {
                *slot = self.sign(side, seg as i32);
            }
        }
        all
    }

    fn cached(signs: &[[Option<Sign>; SEGS]; 2], side: f32, seg: i32) -> Option<Sign> {
        let row = &signs[usize::from(side > 0.0)];
        usize::try_from(seg).ok().and_then(|s| row.get(s).copied().flatten())
    }

    fn sign_at(&self, signs: &[[Option<Sign>; SEGS]; 2], side: f32, z: f32, y: f32) -> Option<(Rgb, f32)> {
        let seg = (z / BUILDING).floor() as i32;
        let (z0, z1, y0, y1, color, level, _) = Self::cached(signs, side, seg)?;
        if z < z0 || z > z1 || y < y0 || y > y1 {
            return None;
        }
        let id = (seg * 2 + if side < 0.0 { 0 } else { 1 }) as u32 ^ self.seed;
        let (u, v) = ((z - z0) / (z1 - z0), (y - y0) / (y1 - y0));
        let border = !(0.08..=0.92).contains(&u) || !(0.04..=0.96).contains(&v);
        let stroke = hash(id ^ (u * 3.0) as u32, (v * 7.0) as u32) < 0.55 && ((u * 6.0).fract() < 0.7);
        let lit = if border { 0.9 } else if stroke { 1.0 } else { 0.15 };
        Some((color, lit * level))
    }

    /// Luz de colores que los letreros tiran alrededor: sobre la pared (`wall` = lado) o el
    /// suelo (`wall` = None, `x` lateral).
    fn spill(signs: &[[Option<Sign>; SEGS]; 2], wall: Option<f32>, x: f32, z: f32, y: f32) -> Rgb {
        let mut light = Rgb::BLACK;
        let seg = (z / BUILDING).floor() as i32;
        for side in [-1.0f32, 1.0] {
            if wall.is_some_and(|w| w != side) {
                continue;
            }
            for s in seg - 1..=seg + 1 {
                let Some((z0, z1, y0, y1, color, _, glow)) = Self::cached(signs, side, s) else { continue };
                let dz = (z0 - z).max(z - z1).max(0.0);
                let dy = (y0 - y).max(y - y1).max(0.0);
                let across = if wall.is_some() { 0.0 } else { (HALF_WIDTH - side * x).max(0.0) };
                let d = (dz * dz + dy * dy + across * across).sqrt();
                light = light.add(color, glow * 0.22 * (-d / 0.45).exp());
            }
        }
        light
    }

    fn wall_color(&self, signs: &[[Option<Sign>; SEGS]; 2], side: f32, z: f32, y: f32) -> Rgb {
        let seg = (z / BUILDING).floor() as i32;
        let id = (seg * 2 + if side < 0.0 { 0 } else { 1 }) as u32 ^ self.seed;
        let tone = 0.09 + 0.06 * hash(id, 8);
        let mut c = Rgb(tone, tone * 0.95, tone * 1.1);
        // Revestimiento: tablas horizontales y uniones verticales, que se ven de cerca.
        if ((y + 3.0) * 4.0).fract() < 0.08 {
            c = c.scale(0.75);
        }
        if (z * 2.0).fract() < 0.05 {
            c = c.scale(0.8);
        }
        // Junta entre edificios.
        if (z / BUILDING).fract() < 0.03 {
            c = c.scale(0.5);
        }
        // Equipos de aire acondicionado colgados de la pared.
        let unit = (z * 0.8).floor();
        if hash(id ^ unit as u32, 31) < 0.35 {
            let (u0, y0) = (unit / 0.8 + 0.3, -0.55 - 0.9 * hash(id ^ unit as u32, 32));
            if (u0..u0 + 0.35).contains(&z) && (y0..y0 + 0.28).contains(&y) {
                let grill = ((z - u0) * 40.0).fract() < 0.5 && (z - u0) > 0.05 && (z - u0) < 0.22;
                c = if grill { Rgb(0.10, 0.10, 0.11) } else { Rgb(0.30, 0.30, 0.31) };
            }
        }
        // Afiches pegados abajo, en la pared izquierda.
        if side < 0.0 && (0.8..1.35).contains(&z) && (-0.45..0.15).contains(&y) {
            let stripe = ((y + 0.45) * 8.0) as i32 % 3;
            c = [Rgb(0.55, 0.20, 0.25), Rgb(0.75, 0.70, 0.55), Rgb(0.20, 0.30, 0.45)][stripe as usize].scale(0.7);
        }
        // Ventanas: algunas encendidas, cálidas.
        let (wu, wv) = ((z * 2.2).fract(), ((y + 3.0) * 2.6).fract());
        if y < -0.6 && wu > 0.25 && wu < 0.75 && wv > 0.3 && wv < 0.8 {
            let lit = hash(id ^ (z * 2.2) as u32, ((y + 3.0) * 2.6) as u32) < 0.35;
            c = if lit { Rgb(1.0, 0.78, 0.45).scale(0.55) } else { Rgb(0.03, 0.03, 0.05) };
        }
        // Máquina expendedora, en la pared derecha, cerca.
        if side > 0.0 && (2.0..2.7).contains(&z) && y > -0.95 {
            let face = Rgb(0.80, 0.90, 1.0).scale(0.75 * self.power().max(0.3));
            let (u, v) = ((z - 2.0) / 0.7, (y + 0.95) / (EYE + 0.95));
            c = if v < 0.55 && u > 0.1 && u < 0.9 && ((u * 5.0).fract() < 0.75) && ((v * 6.0).fract() < 0.6) {
                [Rgb(1.0, 0.3, 0.3), Rgb(0.3, 0.6, 1.0), Rgb(1.0, 0.85, 0.3), Rgb(0.4, 1.0, 0.5)][((u * 5.0) as usize + (v * 6.0) as usize) % 4].scale(0.8)
            } else if v > 0.75 {
                Rgb(0.15, 0.15, 0.18)
            } else {
                face
            };
        }
        // Cañería que baja por la pared izquierda.
        if side < 0.0 && (z - 4.6).abs() < 0.05 {
            c = Rgb(0.18, 0.18, 0.2);
        }
        // Lo de cerca queda en penumbra; los letreros iluminan alrededor.
        c = c.scale(0.55 + 0.45 * smoothstep(0.5, 3.0, z)).add(Self::spill(signs, Some(side), 0.0, z, y), 1.0);
        match self.sign_at(signs, side, z, y) {
            Some((neon, k)) => Rgb(0.03, 0.02, 0.04).add(neon, k),
            None => c,
        }
    }

    fn advance(&mut self, f: &Frame) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        if let Some(t) = &mut self.outage {
            *t += dt / 1.8;
            if *t >= 1.0 {
                self.outage = None;
            }
        }
        if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }
        self.beat = (self.beat * (-dt / 0.25).exp()).max(if b.beat { 1.0 } else { 0.0 });
        self.hat = (self.hat * (-dt / 0.12).exp()).max(if b.hat { 1.0 } else { 0.0 });
        self.climax = (self.climax * (-dt / 1.5).exp()).max(if b.climax { 1.0 } else { 0.0 });

        // Gotas que caen en los charcos: más con la lluvia del ánimo y con el bombo.
        let rate = dt * (6.0 + 10.0 * self.scenery.atmosphere().rain) + if b.kick { 2.0 } else { 0.0 };
        let mut n = rate as usize + usize::from(self.rng.next() < rate.fract());
        while n > 0 && self.ripples.len() < MAX_RIPPLES {
            self.ripples.push(Ripple { x: self.rng.range(-0.95, 0.95), z: self.rng.range(1.6, 9.0), age: 0.0 });
            n -= 1;
        }
        for r in &mut self.ripples {
            r.age += dt;
        }
        self.ripples.retain(|r| r.age < 1.2);

        // Vapor que sale de una rejilla en el suelo.
        if self.steam.len() < MAX_STEAM && self.rng.next() < dt * 6.0 {
            self.steam.push(Puff { x: -0.55 + self.rng.range(-0.05, 0.05), y: EYE, age: 0.0 });
        }
        for p in &mut self.steam {
            p.age += dt;
            p.y -= dt * 0.35;
            p.x += dt * 0.08 * (p.age * 2.0).sin();
        }
        self.steam.retain(|p| p.age < 3.0);
        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    /// Proyecta un punto del callejón (x, y, z) a píxeles.
    fn project(x: f32, y: f32, z: f32, w: f32, h: f32) -> (f32, f32) {
        let f = h * FOCAL;
        (w * VANISH.0 + x / z * f, h * VANISH.1 + y / z * f)
    }

    fn draw_lanterns(&self, canvas: &mut Canvas) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let power = self.power();
        for (i, z) in [1.6f32, 3.4, 5.2, 7.0, 9.4, 11.8].into_iter().enumerate() {
            for side in [-1.0f32, 1.0] {
                if hash(self.seed ^ i as u32, side as i32 as u32) < 0.3 {
                    continue;
                }
                let swing = 0.05 * (self.time * 1.3 + i as f32 + side).sin();
                let (cx, cy) = Self::project(side * (HALF_WIDTH - 0.18) + swing, -1.15, z, w, h);
                let r = (0.13 / z * h * FOCAL).max(0.8);
                let glow = Rgb(1.0, 0.35, 0.2);
                for y in (cy - r * 1.3) as i32..=(cy + r * 1.3) as i32 {
                    for x in (cx - r) as i32..=(cx + r) as i32 {
                        let d = ((x as f32 + 0.5 - cx) / r).powi(2) + ((y as f32 + 0.5 - cy) / (r * 1.3)).powi(2);
                        if d <= 1.0 {
                            let band = if ((y as f32 - cy) / (r * 0.4)).fract().abs() < 0.2 { 0.75 } else { 1.0 };
                            canvas.solid(x, y, Rgb(0.85, 0.15, 0.10).scale((0.3 + 0.7 * power) * band).add(Rgb(1.0, 0.8, 0.5), 0.25 * power * (1.0 - d)));
                        }
                    }
                }
                // Halo alrededor.
                let halo = (r * 3.0) as i32;
                for y in -halo..=halo {
                    for x in -halo..=halo {
                        let d = ((x * x + y * y) as f32).sqrt() / halo.max(1) as f32;
                        if d < 1.0 {
                            canvas.glow(cx as i32 + x, cy as i32 + y, glow, 0.10 * power * (1.0 - d).powi(2));
                        }
                    }
                }
            }
        }
    }

    /// Cables que cruzan el callejón de un techo al otro, con su comba.
    fn draw_wires(&self, canvas: &mut Canvas) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let wire = Rgb(0.03, 0.03, 0.04);
        for (i, (z0, z1)) in [(2.5f32, 3.4f32), (4.8, 4.2), (6.5, 7.6), (9.0, 8.4)].into_iter().enumerate() {
            let (y0, y1) = (-1.7 - 0.2 * (i % 2) as f32, -1.5 - 0.25 * ((i + 1) % 2) as f32);
            let steps = (w * 0.8) as i32;
            for k in 0..=steps {
                let t = k as f32 / steps as f32;
                let sag = 0.25 * (t * std::f32::consts::PI).sin();
                let (x, y) = Self::project(-HALF_WIDTH + 2.0 * HALF_WIDTH * t, y0 + (y1 - y0) * t + sag, z0 + (z1 - z0) * t, w, h);
                canvas.solid(x as i32, y as i32, wire);
            }
        }
    }

    fn draw_cat(&self, canvas: &mut Canvas) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        // Sentado sobre un cajón, a la izquierda y cerca.
        let (z, x) = (2.3f32, -0.62f32);
        let (bx, by) = Self::project(x, EYE, z, w, h);
        let unit = (0.12 / z * h * FOCAL).max(1.0);
        let crate_c = Rgb(0.20, 0.13, 0.08);
        let (cw, ch) = ((unit * 3.2) as i32, (unit * 2.0) as i32);
        for y in 0..ch {
            for dx in -cw / 2..=cw / 2 {
                let slat = if y % 3 == 0 { 0.7 } else { 1.0 };
                canvas.solid(bx as i32 + dx, by as i32 - y, crate_c.scale(slat));
            }
        }
        let top = by - ch as f32;
        let dark = Rgb(0.02, 0.02, 0.025);
        // Cuerpo, cabeza y orejas.
        for y in 0..(unit * 1.8) as i32 {
            let half = (unit * (0.9 - 0.3 * y as f32 / (unit * 1.8))) as i32;
            for dx in -half..=half {
                canvas.solid(bx as i32 + dx, top as i32 - y, dark);
            }
        }
        let hy = top - unit * 2.2;
        for y in -(unit * 0.5) as i32..=(unit * 0.5) as i32 {
            for dx in -(unit * 0.55) as i32..=(unit * 0.55) as i32 {
                canvas.solid(bx as i32 + dx, hy as i32 + y, dark);
            }
        }
        canvas.solid(bx as i32 - (unit * 0.45) as i32, hy as i32 - (unit * 0.6) as i32 - 1, dark);
        canvas.solid(bx as i32 + (unit * 0.45) as i32, hy as i32 - (unit * 0.6) as i32 - 1, dark);
        // Ojos que reflejan el neón.
        canvas.solid(bx as i32 - 1, hy as i32, Rgb(0.6, 0.9, 0.4).scale(0.6));
        canvas.solid(bx as i32 + 1, hy as i32, Rgb(0.6, 0.9, 0.4).scale(0.6));
        // Cola que se mueve con el pulso.
        let wag = (self.time * 2.0).sin() * 0.6 + self.beat * 0.6;
        for k in 0..(unit * 2.0) as i32 {
            let t = k as f32 / (unit * 2.0);
            let x = bx + unit * 0.9 + t * unit * 1.2;
            let y = top - t * unit * 1.4 * wag.abs().max(0.3) - unit * 0.2;
            canvas.solid(x as i32, y as i32, dark);
        }
    }
}

impl Scene for Alley {
    fn update(&mut self, frame: &Frame, _w: usize, _h: usize) {
        self.advance(frame);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        if w < 2.0 || h < 2.0 {
            return;
        }
        let f = h * FOCAL;
        let (vx, vy) = (w * VANISH.0, h * VANISH.1);
        let sky_top = self.scenery.sky(0.0).scale(0.35).mix(Rgb(0.03, 0.03, 0.08), 0.5);
        let sky_low = self.scenery.sky(1.0).scale(0.4).mix(pal.accent, 0.08);
        let street = Rgb(1.0, 0.75, 0.45).mix(pal.petal, 0.25);
        let signs = self.signs();
        // Primero todo lo que no es suelo; después el suelo, que refleja lo de arriba.
        let mut floor_px: Vec<(i32, i32, f32, f32)> = Vec::new();
        for y in 0..canvas.px_h() as i32 {
            for x in 0..canvas.w as i32 {
                let (dx, dy) = ((x as f32 + 0.5 - vx) / f, (y as f32 + 0.5 - vy) / f);
                match self.trace(dx, dy) {
                    Hit::Sky => {
                        let v = (y as f32 / vy).clamp(0.0, 1.0);
                        canvas.solid(x, y, sky_top.mix(sky_low, v));
                    }
                    Hit::Wall(side, z, wy) => {
                        // Lo lejano se pierde en la bruma de la calle del fondo.
                        let fog = smoothstep(3.0, FAR, z) * 0.6;
                        let c = self.wall_color(&signs, side, z, wy).mix(street.scale(0.25), fog);
                        canvas.solid(x, y, c);
                    }
                    Hit::End(ex, ey) => {
                        let k = (1.0 - ex.abs() / HALF_WIDTH).max(0.0) * smoothstep(-2.0, 0.6, ey);
                        canvas.solid(x, y, street.scale(0.25 + 0.5 * k * self.power().max(0.4)));
                    }
                    Hit::Floor(fx, z) => floor_px.push((x, y, fx, z)),
                }
            }
        }
        // Suelo mojado: refleja lo que hay sobre la línea donde se junta con la pared, con
        // ondas de las gotas y charcos más brillantes que el asfalto.
        let asphalt = Rgb(0.025, 0.025, 0.03);
        for &(x, y, fx, z) in &floor_px {
            let dx = (x as f32 + 0.5 - vx) / f;
            // Donde la pared (o el fondo) toca el suelo en esta columna.
            let junction = vy + f * EYE * (dx.abs() / HALF_WIDTH).max(1.0 / FAR);
            // Solo se refleja lo que no es suelo (lo de arriba de la junta).
            let mut ry = (junction - (y as f32 - junction)).min(junction - 1.0);
            // Las ondas mueven el reflejo.
            let mut wobble = 0.0;
            for r in &self.ripples {
                let d = ((fx - r.x).powi(2) + ((z - r.z) * 0.6).powi(2)).sqrt();
                let front = r.age * 0.5;
                let k = (1.0 - (d - front).abs() / 0.06).max(0.0) * (1.0 - r.age / 1.2);
                wobble += k;
            }
            ry = (ry + wobble * 2.0 * (self.time * 20.0).sin()).min(junction - 1.0);
            // Charcos: reflejan bastante; el asfalto alrededor, apenas.
            let puddle = 0.12 + 0.43 * smoothstep(0.45, 0.65, hash((fx * 3.0 + 10.0) as u32 ^ self.seed, (z * 1.5) as u32));
            let reflected = canvas.pixel(x, ry as i32).unwrap_or(asphalt);
            let c = asphalt
                .mix(reflected, puddle * (1.0 - smoothstep(4.0, FAR, z) * 0.5))
                .add(Self::spill(&signs, None, fx, z, EYE), 0.6)
                .add(Rgb::WHITE, 0.12 * wobble.min(1.0));
            canvas.solid(x, y, c);
        }
        self.draw_wires(canvas);
        self.draw_lanterns(canvas);
        self.draw_cat(canvas);
        // Vapor de la rejilla.
        for p in &self.steam {
            let (sx, sy) = Self::project(p.x, p.y, 2.8, w, h);
            let k = (1.0 - p.age / 3.0) * 0.25;
            let r = (1.0 + p.age) as i32;
            for dy in -r..=r {
                for dx in -r..=r {
                    let d = ((dx * dx + dy * dy) as f32).sqrt() / (r as f32 + 0.5);
                    canvas.paint(sx as i32 + dx, sy as i32 + dy, Rgb(0.75, 0.75, 0.8), k * (1.0 - d).max(0.0));
                }
            }
        }
        // Lluvia: trazos finos y oblicuos, que se ven contra la luz.
        let drops = (w * h * 0.012) as u32;
        for i in 0..drops {
            let speed = 1.2 + hash(i, 2);
            let x = (hash(i, 1) * w + self.time * 3.0) % w;
            let y = ((hash(i, 3) + self.time * speed) % 1.0) * h;
            for k in 0..3 {
                canvas.paint(x as i32 - k / 2, y as i32 - k, Rgb(0.65, 0.70, 0.85), 0.22 - 0.05 * k as f32);
            }
        }
        if self.climax > 0.05 {
            canvas.wash(Rgb(1.0, 0.4, 0.7), 0.06 * self.climax);
        }
    }

    fn new_track(&mut self) {
        if self.outage.is_none() {
            self.outage = Some(0.0);
        }
        self.scenery.fade();
    }

    /// Un click en el suelo hace una onda; en cualquier otro lado, también, cerca del gato.
    fn poke(&mut self, x: f32, y: f32) {
        let dy = y - VANISH.1;
        let z = if dy > 0.01 { (EYE / (dy / FOCAL)).clamp(1.2, 9.0) } else { 2.5 };
        let fx = ((x - VANISH.0) / FOCAL * z).clamp(-0.95, 0.95);
        for k in 0..3 {
            if self.ripples.len() < MAX_RIPPLES + 6 {
                self.ripples.push(Ripple { x: fx, z, age: -0.15 * k as f32 });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(a: &mut Alley, bands: &Bands, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            a.update(&Frame { dt: 1.0 / 60.0, bands, progress: Some(0.5), song_len: 120.0, playing: true }, 120, 40);
        }
    }

    #[test]
    fn rays_see_walls_floor_sky_and_the_far_street() {
        let a = Alley::new(1);
        assert!(matches!(a.trace(-0.6, 0.0), Hit::Wall(s, ..) if s < 0.0));
        assert!(matches!(a.trace(0.6, 0.0), Hit::Wall(s, ..) if s > 0.0));
        assert!(matches!(a.trace(0.0, 0.5), Hit::Floor(..)));
        assert!(matches!(a.trace(0.0, -0.9), Hit::Sky));
        assert!(matches!(a.trace(0.01, 0.01), Hit::End(..)));
    }

    #[test]
    fn rain_makes_ripples_and_a_new_song_cuts_the_power() {
        let mut a = Alley::new(2);
        run(&mut a, &Bands::default(), 1.0);
        assert!(!a.ripples.is_empty());
        a.new_track();
        run(&mut a, &Bands::default(), 0.3);
        assert!(a.power() < 0.2);
        run(&mut a, &Bands::default(), 2.0);
        assert_eq!(a.power(), 1.0);
    }

    #[test]
    fn a_click_on_the_floor_ripples_there() {
        let mut a = Alley::new(3);
        a.ripples.clear();
        a.poke(0.5, 0.9);
        assert!(a.ripples.len() >= 3);
        assert!(a.ripples.iter().all(|r| r.x.abs() < 0.2 && r.z < 3.0));
    }

    #[test]
    fn draws_at_any_size() {
        let mut a = Alley::new(4);
        let bands = Bands { beat: true, hat: true, kick: true, climax: true, ..Bands::default() };
        run(&mut a, &bands, 1.0);
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (220, 60)] {
            let mut canvas = Canvas::new(w, h);
            a.draw(&mut canvas, &Palette::default());
        }
    }
}

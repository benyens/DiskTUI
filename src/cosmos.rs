//! Escena sistema solar: primero aparecen estrellas, luego nace un sol, luego planetas.
//!
//! Coordenadas de mundo: origen en el sol, `y` hacia arriba, órbita exterior de radio 1.
//! Las órbitas se ven inclinadas: el eje vertical se aplasta por `TILT`.

use std::f32::consts::TAU;

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::mood::Mood;
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::scene::{Frame, Scene};

/// Aplastamiento vertical de las órbitas (vista inclinada).
const TILT: f32 = 0.36;
const SUN_RADIUS: f32 = 0.10;
const MAX_STARS: usize = 240;
/// Tramos de la canción (0..1): cuándo aparece cada cosa.
const STARS_FULL_AT: f32 = 0.20;
const SUN_BORN: (f32, f32) = (0.05, 0.20);
const PLANETS_FROM: f32 = 0.20;
const PLANETS_SPAN: f32 = 0.48;

struct Star {
    /// Posición como fracción del lienzo, para sobrevivir cambios de tamaño.
    fx: f32,
    fy: f32,
    mag: f32,
    phase: f32,
    /// Estrella grande con destello.
    bright: bool,
    /// 0 azulada, 1 color claro de la carátula, 2 acento.
    tint: u8,
    /// Momento de aparición; `None` = aún no nace.
    born: Option<f32>,
    flash: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Rocky,
    Gas,
    Ice,
}

struct Planet {
    orbit: f32,
    angle: f32,
    /// Velocidad angular base, rad/s.
    speed: f32,
    radius: f32,
    kind: Kind,
    /// Qué color de la paleta usa de base.
    role: u8,
    hue: f32,
    ring: bool,
    moons: u8,
    /// Avance de la canción en que nace.
    birth: f32,
    /// Segundos desde que nació; negativo = aún no.
    age: f32,
    phase: f32,
}

struct Rock {
    orbit: f32,
    angle: f32,
    speed: f32,
}

struct Comet {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    life: f32,
}

/// Onda que sale del sol con cada bombo (tenue) o clímax (fuerte).
struct Wave {
    r: f32,
    strength: f32,
    speed: f32,
}

pub struct Cosmos {
    time: f32,
    rng: Rng,
    /// Avance de la canción, 0..1.
    age: f32,
    /// Reloj propio, por si no se conoce el avance.
    clock: f32,
    bands: Bands,
    kick_pulse: f32,
    hat_pulse: f32,
    climax_pulse: f32,
    stars: Vec<Star>,
    /// Estrellas extra encendidas por los platillos.
    bonus_stars: f32,
    planets: Vec<Planet>,
    belt: Vec<Rock>,
    belt_from: f32,
    comets: Vec<Comet>,
    waves: Vec<Wave>,
    /// Giro de todo el plano orbital, radianes.
    lean: f32,
    /// Nebulosas: (x, y, radio, rol de color), en fracción del lienzo.
    nebulae: [(f32, f32, f32, u8); 3],
    /// Colapso al cambiar de canción: 0..1.
    collapse: Option<f32>,
    /// Ánimo de la canción, suavizado: (valencia, activación).
    feeling: (f32, f32),
    forced: Option<Mood>,
}

impl Cosmos {
    pub fn new(seed: u64) -> Self {
        let mut cosmos = Cosmos {
            time: 0.0,
            rng: Rng::new(seed),
            age: 0.0,
            clock: 0.0,
            bands: Bands::default(),
            kick_pulse: 0.0,
            hat_pulse: 0.0,
            climax_pulse: 0.0,
            stars: Vec::new(),
            bonus_stars: 0.0,
            planets: Vec::new(),
            belt: Vec::new(),
            belt_from: 0.6,
            comets: Vec::new(),
            waves: Vec::new(),
            lean: 0.0,
            nebulae: [(0.0, 0.0, 0.0, 0); 3],
            collapse: None,
            feeling: (0.5, 0.5),
            forced: None,
        };
        cosmos.generate();
        cosmos
    }

    /// Crea un sistema nuevo a partir del estado del generador.
    fn generate(&mut self) {
        let rng = &mut self.rng;
        self.age = 0.0;
        self.clock = 0.0;
        self.bonus_stars = 0.0;
        self.collapse = None;
        self.comets.clear();
        self.waves.clear();
        self.lean = rng.range(-0.16, 0.16);

        self.stars = (0..MAX_STARS)
            .map(|_| {
                let roll = rng.next();
                Star {
                    fx: rng.next(),
                    fy: rng.next(),
                    mag: 0.25 + 0.75 * rng.next().powf(2.2),
                    phase: rng.range(0.0, TAU),
                    bright: roll < 0.07,
                    tint: if roll > 0.86 { 2 } else if roll > 0.68 { 1 } else { 0 },
                    born: None,
                    flash: 0.0,
                }
            })
            .collect();

        // Entre 4 y 7 planetas. Rocosos adentro, gigantes y hielo afuera.
        let count = 4 + (rng.next() * 4.0) as usize;
        self.planets = (0..count)
            .map(|i| {
                let f = i as f32 / (count - 1) as f32;
                let orbit = 0.27 + 0.73 * f + rng.range(-0.02, 0.02);
                let roll = rng.next();
                let kind = if f < 0.35 {
                    Kind::Rocky
                } else if roll < 0.55 {
                    Kind::Gas
                } else if roll < 0.8 {
                    Kind::Ice
                } else {
                    Kind::Rocky
                };
                let radius = match kind {
                    Kind::Rocky => rng.range(0.017, 0.030),
                    Kind::Gas => rng.range(0.042, 0.062),
                    Kind::Ice => rng.range(0.028, 0.040),
                };
                Planet {
                    orbit,
                    angle: rng.range(0.0, TAU),
                    speed: 0.12 * orbit.powf(-1.5), // Kepler: los de adentro giran más rápido
                    radius,
                    kind,
                    role: (rng.next() * 6.0) as u8,
                    hue: rng.range(-0.05, 0.05),
                    ring: kind != Kind::Rocky && rng.next() < 0.45,
                    moons: if kind == Kind::Rocky { (rng.next() * 1.4) as u8 } else { (rng.next() * 2.6) as u8 },
                    birth: PLANETS_FROM + PLANETS_SPAN * (i as f32 + rng.range(0.0, 0.5)) / count as f32,
                    age: -1.0,
                    phase: rng.range(0.0, TAU),
                }
            })
            .collect();

        // Cinturón de asteroides entre dos órbitas del medio.
        let k = (count / 2).min(count - 2);
        let belt_orbit = (self.planets[k].orbit + self.planets[k + 1].orbit) / 2.0;
        self.belt_from = (self.planets[k].birth + self.planets[k + 1].birth) / 2.0;
        self.belt = (0..90)
            .map(|_| {
                let orbit = belt_orbit + rng.range(-0.022, 0.022);
                Rock { orbit, angle: rng.range(0.0, TAU), speed: 0.12 * orbit.powf(-1.5) * rng.range(0.9, 1.1) }
            })
            .collect();

        for n in &mut self.nebulae {
            *n = (rng.range(0.1, 0.9), rng.range(0.1, 0.9), rng.range(0.22, 0.42), (rng.next() * 3.0) as u8);
        }
    }

    /// Cuánto ha nacido el sol, 0..1.
    fn sun_level(&self) -> f32 {
        smoothstep(SUN_BORN.0, SUN_BORN.1, self.age) * self.shrink()
    }

    /// Factor de colapso: 1 normal, 0 todo cayó al centro.
    fn shrink(&self) -> f32 {
        self.collapse.map(|t| 1.0 - smoothstep(0.0, 1.0, t)).unwrap_or(1.0)
    }

    #[cfg(test)]
    fn planets_born(&self) -> usize {
        self.planets.iter().filter(|p| p.age >= 0.0).count()
    }

    /// Fija el ánimo en vez de leerlo de la canción.
    pub fn force_mood(&mut self, mood: Option<Mood>) {
        self.forced = mood;
    }

    /// Qué tan triste (lluvia de estrellas) y qué tan intensa (erupciones) es la canción.
    fn sorrow(&self) -> f32 {
        smoothstep(0.50, 0.25, self.feeling.0) * smoothstep(0.55, 0.30, self.feeling.1)
    }

    fn fury(&self) -> f32 {
        smoothstep(0.50, 0.25, self.feeling.0) * smoothstep(0.50, 0.75, self.feeling.1)
    }

    /// Tinte del espacio: frío si la canción es triste, cálido si es alegre.
    fn tint(&self) -> Rgb {
        Rgb(0.20, 0.42, 1.0).mix(Rgb(1.0, 0.62, 0.30), smoothstep(0.2, 0.8, self.feeling.0))
    }

    fn advance(&mut self, f: &Frame) {
        let dt = f.dt;
        let b = f.bands;
        self.time += dt;
        self.bands = *b;
        let (target, pull) = match self.forced {
            Some(mood) => (mood.center(), 1.0),
            None => ((b.song.valence, b.song.arousal), b.song.settled),
        };
        let k = (1.0 - (-dt / 2.0).exp()) * pull;
        self.feeling = (self.feeling.0 + (target.0 - self.feeling.0) * k, self.feeling.1 + (target.1 - self.feeling.1) * k);
        let decay = |v: f32, tau: f32, hit: bool| (v * (-dt / tau).exp()).max(if hit { 1.0 } else { 0.0 });
        self.kick_pulse = decay(self.kick_pulse, 0.20, b.kick);
        self.hat_pulse = decay(self.hat_pulse, 0.12, b.hat);
        self.climax_pulse = decay(self.climax_pulse, 2.5, b.climax);

        if let Some(t) = &mut self.collapse {
            *t += dt / 1.4;
            if *t >= 1.0 {
                self.generate();
            }
        } else {
            if f.playing {
                self.clock += dt;
            }
            self.age = f.progress.unwrap_or((self.clock / f.song_len.max(20.0)).min(1.0));
        }

        // Estrellas: aparecen de a poco; cada platillo enciende una más y hace destellar otra.
        if b.hat {
            self.bonus_stars += 1.0;
            let born: Vec<usize> = (0..self.stars.len()).filter(|i| self.stars[*i].born.is_some()).collect();
            if !born.is_empty() {
                let pick = born[(self.rng.next() * born.len() as f32) as usize % born.len()];
                self.stars[pick].flash = 1.0;
            }
        }
        let wanted = 6.0
            + MAX_STARS as f32 * 0.8 * smoothstep(0.0, STARS_FULL_AT, self.age).powf(0.8)
            + self.bonus_stars;
        let time = self.time;
        for (i, star) in self.stars.iter_mut().enumerate() {
            if star.born.is_none() && (i as f32) < wanted {
                star.born = Some(time);
            }
            star.flash *= (-dt / 0.25).exp();
        }

        // Planetas: nacen por turno y giran más rápido con los medios y la intensidad.
        // Las canciones enérgicas giran más rápido; las calmadas, más lento.
        let pace = 0.55 + 0.9 * self.feeling.1;
        let tempo = if f.playing { pace * (0.35 + 0.8 * b.mid + 0.9 * b.intensity + 0.6 * self.climax_pulse) } else { 0.04 };
        let age = self.age;
        for p in &mut self.planets {
            if p.age < 0.0 && age >= p.birth {
                p.age = 0.0;
            } else if p.age >= 0.0 {
                p.age += dt;
            }
            p.angle = (p.angle + p.speed * tempo * dt).rem_euclid(TAU);
        }
        for r in &mut self.belt {
            r.angle = (r.angle + r.speed * tempo * dt).rem_euclid(TAU);
        }

        // Ondas desde el sol.
        let sun = self.sun_level();
        if b.kick && sun > 0.3 {
            self.waves.push(Wave { r: SUN_RADIUS, strength: 0.25 + 0.5 * b.bass, speed: 0.55 });
        }
        // Canción intensa: el sol lanza una erupción con cada pulso.
        if b.beat && sun > 0.3 && self.fury() > 0.4 {
            self.waves.push(Wave { r: SUN_RADIUS, strength: 0.5 * self.fury(), speed: 0.9 });
        }
        if b.climax && sun > 0.1 {
            self.waves.push(Wave { r: SUN_RADIUS, strength: 1.6, speed: 0.75 });
            for _ in 0..7 {
                self.spawn_comet();
            }
        }
        for w in &mut self.waves {
            w.r += w.speed * dt;
        }
        self.waves.retain(|w| w.r < 1.5);

        // Cometas: algunos platillos lanzan uno.
        if b.hat && self.age > 0.04 && self.rng.next() < 0.10 + 0.25 * b.intensity {
            self.spawn_comet();
        }
        // Canción triste: lluvia de estrellas, haya platillos o no.
        if f.playing && self.age > 0.04 && self.rng.next() < 1.6 * self.sorrow() * dt {
            self.spawn_comet();
        }
        for c in &mut self.comets {
            c.age += dt;
            c.x += c.vx * dt;
            c.y += c.vy * dt;
        }
        self.comets.retain(|c| c.age < c.life);
    }

    fn spawn_comet(&mut self) {
        if self.comets.len() >= 16 {
            return;
        }
        // Entra desde el borde y cruza cerca del centro, sin apuntar justo al sol.
        let from = self.rng.range(0.0, TAU);
        let (x, y) = (from.cos() * 1.5, from.sin() * 0.75);
        let (tx, ty) = (self.rng.range(-0.6, 0.6), self.rng.range(-0.3, 0.3));
        let d = ((tx - x).powi(2) + (ty - y).powi(2)).sqrt().max(0.01);
        let v = self.rng.range(1.3, 2.3);
        self.comets.push(Comet {
            x,
            y,
            vx: (tx - x) / d * v,
            vy: (ty - y) / d * v,
            age: 0.0,
            life: self.rng.range(0.7, 1.3),
        });
    }
}

impl Scene for Cosmos {
    fn update(&mut self, frame: &Frame, _w: usize, _h: usize) {
        self.advance(frame);
    }

    fn new_track(&mut self) {
        if self.collapse.is_none() {
            self.collapse = Some(0.0);
        }
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let view = View::new(canvas);
        let b = &self.bands;
        let ph = canvas.px_h().max(1) as f32;
        let sun = self.sun_level();
        let sun_color = pal.petal.mix(pal.accent, 0.25);

        // Fondo: espacio más oscuro que el del jardín; destella en los clímax.
        let tint = self.tint();
        let lift = 0.012 * b.intensity + 0.06 * self.climax_pulse;
        canvas.clear(|_, y| {
            pal.bg_top.scale(0.45).mix(pal.bg_bottom.scale(0.6), y as f32 / ph).add(sun_color, lift).add(tint, 0.035)
        });
        self.draw_nebulae(canvas, pal);
        self.draw_corona(canvas, &view, sun, sun_color);

        // Trazo fino: órbitas, estelas, cinturón, ondas y cometas.
        self.draw_orbits(canvas, &view, pal);
        self.draw_belt(canvas, &view, pal);
        self.draw_waves(canvas, &view, pal);
        self.draw_comets(canvas, &view, pal);

        // Planetas del fondo, sol, planetas del frente.
        for p in self.planets.iter().filter(|p| p.age >= 0.0 && p.angle.sin() > 0.0) {
            self.draw_planet(canvas, &view, p, pal);
        }
        self.draw_sun(canvas, &view, sun, sun_color, pal);
        for p in self.planets.iter().filter(|p| p.age >= 0.0 && p.angle.sin() <= 0.0) {
            self.draw_planet(canvas, &view, p, pal);
        }

        self.draw_stars(canvas, pal);
    }
}

/// Transformación de mundo a píxeles, centrada en el sol.
struct View {
    cx: f32,
    cy: f32,
    scale: f32,
}

impl View {
    fn new(canvas: &Canvas) -> Self {
        let (w, ph) = (canvas.w as f32, canvas.px_h() as f32);
        let scale = (w / 2.0 / 1.14).min(ph / 2.0 / (TILT + 0.14)).max(4.0);
        View { cx: w / 2.0, cy: ph / 2.0, scale }
    }

    /// Punto de mundo (sin inclinar) a píxeles.
    fn px(&self, x: f32, y: f32) -> (f32, f32) {
        (self.cx + x * self.scale, self.cy - y * self.scale)
    }
}

impl Cosmos {
    /// Punto de una órbita de radio `orbit` en el ángulo `angle`, en mundo.
    fn orbit_point(&self, orbit: f32, angle: f32) -> (f32, f32) {
        let k = self.shrink();
        let (x, y) = (orbit * k * angle.cos(), orbit * k * angle.sin() * TILT);
        let (s, c) = self.lean.sin_cos();
        (x * c - y * s, x * s + y * c)
    }

    fn planet_color(&self, p: &Planet, pal: &Palette) -> (Rgb, Rgb) {
        let base = match p.role {
            0 => pal.accent,
            1 => pal.leaf,
            2 => pal.petal_light,
            3 => pal.petal,
            4 => pal.stem.mix(Rgb::WHITE, 0.25),
            _ => pal.soil.mix(pal.accent, 0.4),
        };
        let (h, s, l) = base.to_hsl();
        let h = h + p.hue;
        match p.kind {
            Kind::Rocky => (Rgb::from_hsl(h, s * 0.6, 0.60), Rgb::from_hsl(h + 0.03, s * 0.45, 0.44)),
            Kind::Gas => (Rgb::from_hsl(h, s.max(0.45), l.clamp(0.52, 0.66)), Rgb::from_hsl(h + 0.06, s.max(0.4), 0.42)),
            Kind::Ice => {
                let ice = Rgb(0.72, 0.88, 1.0);
                (Rgb::from_hsl(h, s * 0.6, 0.66).mix(ice, 0.55), Rgb::from_hsl(h, s * 0.5, 0.5).mix(ice, 0.3))
            }
        }
    }

    fn draw_nebulae(&self, canvas: &mut Canvas, pal: &Palette) {
        let presence = smoothstep(0.02, 0.35, self.age) * self.shrink();
        let strength = presence * (0.035 + 0.06 * self.bands.intensity + 0.05 * self.climax_pulse);
        if strength < 0.004 {
            return;
        }
        let (w, ph) = (canvas.w as f32, canvas.px_h() as f32);
        let blobs: Vec<(f32, f32, f32, Rgb)> = self
            .nebulae
            .iter()
            .enumerate()
            .map(|(i, (x, y, r, role))| {
                let drift = (self.time * 0.05 + i as f32 * 2.1).sin() * 0.03;
                let color = [pal.petal, pal.accent, pal.leaf][*role as usize % 3].mix(self.tint(), 0.40);
                ((x + drift) * w, y * ph, r * w.max(ph * 2.0) * 0.5, color)
            })
            .collect();
        for y in 0..canvas.px_h() {
            for x in 0..canvas.w {
                for (bx, by, r, color) in &blobs {
                    // Píxeles cuadrados: sin corrección de aspecto.
                    let d2 = ((x as f32 - bx).powi(2) + (y as f32 - by).powi(2)) / (r * r);
                    if d2 < 4.0 {
                        canvas.glow(x as i32, y as i32, *color, strength * (-d2 * 1.6).exp());
                    }
                }
            }
        }
    }

    fn sun_radius_px(&self, view: &View, level: f32) -> f32 {
        let b = &self.bands;
        SUN_RADIUS
            * view.scale
            * level
            * (1.0 + 0.05 * b.bass + 0.05 * self.kick_pulse + 0.10 * self.climax_pulse + 0.04 * b.intensity)
            // Late al compás: se hincha en cada pulso previsto y se relaja hasta el siguiente.
            * (1.0 + 0.05 * (1.0 - b.beat_phase).powi(2) * b.song.tempo_confidence)
    }

    /// Corona: resplandor con rayos que giran lento. Antes de nacer el sol, es solo un brillo.
    fn draw_corona(&self, canvas: &mut Canvas, view: &View, level: f32, color: Rgb) {
        let b = &self.bands;
        // El brillo aparece antes que el disco: primero se ve una nube que se enciende.
        let glow = smoothstep(SUN_BORN.0 - 0.05, SUN_BORN.1, self.age) * self.shrink();
        if glow < 0.01 {
            return;
        }
        let r = self.sun_radius_px(view, level.max(0.25));
        let power = glow * (0.22 + 0.40 * b.level + 0.25 * self.kick_pulse + 0.55 * self.climax_pulse);
        let reach = r * 4.5;
        let (x0, x1) = ((view.cx - reach).floor() as i32, (view.cx + reach).ceil() as i32);
        let (y0, y1) = ((view.cy - reach).floor() as i32, (view.cy + reach).ceil() as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (dx, dy) = (x as f32 + 0.5 - view.cx, y as f32 + 0.5 - view.cy);
                let d = (dx * dx + dy * dy).sqrt();
                let theta = dy.atan2(dx);
                let rays = 1.0
                    + 0.35 * (theta * 6.0 + self.time * 0.5).sin() * (theta * 11.0 - self.time * 0.3).sin()
                        * (0.4 + b.level);
                let k = power * (-(d - r).max(0.0) / (r * 0.95)).exp() * rays;
                if k > 0.004 {
                    canvas.glow(x, y, color, k);
                }
            }
        }
    }

    fn draw_sun(&self, canvas: &mut Canvas, view: &View, level: f32, color: Rgb, pal: &Palette) {
        let r = self.sun_radius_px(view, level);
        if r < 0.5 {
            return;
        }
        let core = color.mix(Rgb::WHITE, 0.82);
        let limb = color.scale(0.80);
        let t = self.time;
        disc(canvas, view.cx, view.cy, r, |dx, dy| {
            let d = (dx * dx + dy * dy).sqrt() / r;
            // Granulación: manchas que se mueven lento por la superficie.
            let grain = (dx * 0.9 + t * 1.1).sin() * (dy * 1.2 - t * 0.8).sin() + ((dx + dy) * 0.5 + t * 0.6).sin();
            core.mix(color, d.powf(1.4))
                .mix(limb, smoothstep(0.7, 1.0, d))
                .add(pal.accent, 0.07 * grain)
        });
    }

    /// Cuánto iluminan las ondas a un radio dado, 0..1.
    fn wave_light(&self, orbit: f32) -> f32 {
        self.waves
            .iter()
            .map(|w| w.strength * (-((orbit - w.r) / 0.05).powi(2)).exp())
            .sum::<f32>()
            .min(1.0)
    }

    fn draw_orbits(&self, canvas: &mut Canvas, view: &View, pal: &Palette) {
        let dot = |canvas: &mut Canvas, (x, y): (f32, f32), color: Rgb, alpha: f32| {
            let (px, py) = view.px(x, y);
            canvas.dot_over((px * 2.0).round() as i32, (py * 2.0).round() as i32, color, alpha);
        };
        for p in self.planets.iter().filter(|p| p.age >= 0.0) {
            let (color, _) = self.planet_color(p, pal);
            // Los planetas oscuros igual dejan una órbita visible.
            let bright = color.mix(Rgb(0.75, 0.78, 0.9), 0.35);
            let fade = smoothstep(0.0, 2.5, p.age);
            let lit = self.wave_light(p.orbit);
            // Línea punteada.
            let steps = (TAU * p.orbit * view.scale).ceil().max(8.0) as usize;
            for i in 0..steps {
                let point = self.orbit_point(p.orbit, TAU * i as f32 / steps as f32);
                dot(canvas, point, bright.mix(pal.accent, 0.8 * lit), (0.36 + 0.6 * lit) * fade);
            }
            // Estela detrás del planeta: más larga cuando la música va intensa.
            let trail = 0.25 + 0.5 * self.bands.intensity + 0.3 * self.bands.mid;
            let dense = (trail * p.orbit * view.scale * 2.2).ceil().max(2.0) as usize;
            for i in 0..dense {
                let u = i as f32 / dense as f32;
                let point = self.orbit_point(p.orbit, p.angle - trail * u);
                dot(canvas, point, bright.mix(Rgb::WHITE, 0.25), (1.0 - u).powf(1.3) * fade);
            }
        }
    }

    fn draw_belt(&self, canvas: &mut Canvas, view: &View, pal: &Palette) {
        let fade = smoothstep(self.belt_from, self.belt_from + 0.06, self.age);
        if fade < 0.02 {
            return;
        }
        let rock = pal.soil.mix(Rgb::WHITE, 0.55);
        for r in &self.belt {
            let lit = self.wave_light(r.orbit);
            let (x, y) = self.orbit_point(r.orbit, r.angle);
            let (px, py) = view.px(x, y);
            canvas.dot_over(
                (px * 2.0).round() as i32,
                (py * 2.0).round() as i32,
                rock.mix(pal.accent, 0.7 * lit),
                (0.6 + 0.4 * lit) * fade,
            );
        }
    }

    /// Solo las ondas de clímax se dibujan; las de bombo solo iluminan las órbitas.
    fn draw_waves(&self, canvas: &mut Canvas, view: &View, pal: &Palette) {
        let color = pal.accent.mix(Rgb::WHITE, 0.4);
        for w in self.waves.iter().filter(|w| w.strength > 1.0) {
            let fade = 1.0 - smoothstep(0.5, 1.5, w.r);
            let steps = (TAU * w.r * view.scale * 2.0).ceil().max(8.0) as usize;
            for i in 0..steps {
                // Las ondas no se encogen con el colapso: se deshace la reducción.
                let (x, y) = self.orbit_point(w.r / self.shrink().max(0.01), TAU * i as f32 / steps as f32);
                let (px, py) = view.px(x, y);
                canvas.dot_over((px * 2.0).round() as i32, (py * 2.0).round() as i32, color, fade);
            }
        }
    }

    fn draw_comets(&self, canvas: &mut Canvas, view: &View, pal: &Palette) {
        let head = pal.accent.mix(Rgb::WHITE, 0.7);
        for c in &self.comets {
            let t = c.age / c.life;
            let fade = smoothstep(0.0, 0.15, t) * (1.0 - smoothstep(0.6, 1.0, t));
            // Cola: del núcleo hacia atrás, apagándose.
            let length = 0.11 * view.scale * 2.0 * (c.vx * c.vx + c.vy * c.vy).sqrt();
            let steps = length.ceil().max(2.0) as usize;
            for i in 0..steps {
                let u = i as f32 / steps as f32;
                let (px, py) = view.px(c.x - c.vx * 0.11 * u, c.y - c.vy * 0.11 * u);
                canvas.dot_over(
                    (px * 2.0).round() as i32,
                    (py * 2.0).round() as i32,
                    head,
                    fade * (1.0 - u).powf(1.5),
                );
            }
            let (hx, hy) = view.px(c.x, c.y);
            if !canvas.covered(hx.floor() as i32, (hy / 2.0).floor() as i32) {
                canvas.glyph(hx.floor() as i32, (hy / 2.0).floor() as i32, '✦', head, fade);
            }
        }
    }

    fn draw_planet(&self, canvas: &mut Canvas, view: &View, p: &Planet, pal: &Palette) {
        let grow = 1.0 - (1.0 - (p.age / 2.5).clamp(0.0, 1.0)).powi(3);
        let (wx, wy) = self.orbit_point(p.orbit, p.angle);
        let (cx, cy) = view.px(wx, wy);
        let r = (p.radius * view.scale * grow * (1.0 + 0.10 * self.kick_pulse * self.bands.bass + 0.08 * self.climax_pulse))
            .max(0.7 * grow);
        if r < 0.3 {
            return;
        }
        let (base, alt) = self.planet_color(p, pal);
        // Luz: desde el sol (en el origen) hacia el planeta, un poco hacia el espectador.
        let dist = (wx * wx + wy * wy).sqrt().max(0.001);
        let light = normalize((-wx / dist, -wy / dist, 0.75));
        let lit_by_wave = self.wave_light(p.orbit);

        let ring_color = base.mix(Rgb::WHITE, 0.45);
        if p.ring && r >= 2.0 {
            self.draw_ring(canvas, cx, cy, r, ring_color);
        }
        let moon_color = Rgb(0.78, 0.78, 0.82);
        for m in 0..p.moons {
            let a = self.time * (1.6 + 0.7 * m as f32) + p.phase + m as f32 * 2.4;
            if a.sin() > 0.0 {
                self.draw_moon(canvas, cx, cy, r, m, a, moon_color);
            }
        }

        // Halo de atmósfera.
        let halo = if p.kind == Kind::Rocky { 0.05 } else { 0.11 };
        glow_disc(canvas, cx, cy, r * 2.2, |d| halo * (-((d - r).max(0.0) / (r * 0.5)).powi(2)).exp(), base);

        let phase = p.phase;
        let kind = p.kind;
        let spin = self.time * 0.25;
        disc(canvas, cx, cy, r, |dx, dy| {
            let (nx, ny) = (dx / r, -dy / r);
            let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
            let diffuse = (nx * light.0 + ny * light.1 + nz * light.2).max(0.0);
            let surface = match kind {
                // Franjas horizontales que se deslizan.
                Kind::Gas => base.mix(alt, 0.5 + 0.5 * (ny * 6.5 + phase + (nx * 2.0 + spin).sin() * 0.6).sin()),
                // Manchas fijas a la superficie.
                Kind::Rocky => base.mix(alt, 0.5 + 0.5 * ((nx * 4.0 + phase + spin).sin() * (ny * 5.0 - phase).cos())),
                Kind::Ice => base.mix(alt, 0.35 * (ny * 3.0 + phase).sin().abs()),
            };
            let specular = if kind == Kind::Ice { diffuse.powi(12) * 0.5 } else { 0.0 };
            surface
                .scale(0.28 + 0.85 * diffuse)
                .add(Rgb::WHITE, specular)
                .mix(pal.accent, 0.35 * lit_by_wave)
        });

        for m in 0..p.moons {
            let a = self.time * (1.6 + 0.7 * m as f32) + p.phase + m as f32 * 2.4;
            if a.sin() <= 0.0 {
                self.draw_moon(canvas, cx, cy, r, m, a, moon_color);
            }
        }
    }

    /// Anillos en Braille, antes del cuerpo: el planeta opaco tapa lo que le pasa por detrás.
    fn draw_ring(&self, canvas: &mut Canvas, cx: f32, cy: f32, r: f32, color: Rgb) {
        let (s, c) = self.lean.sin_cos();
        for (k, radius) in [1.95f32, 1.6].into_iter().enumerate() {
            if k == 1 && r < 3.5 {
                continue;
            }
            let steps = (TAU * radius * r * 2.5).ceil().max(12.0) as usize;
            for i in 0..steps {
                let a = TAU * i as f32 / steps as f32;
                let (x, y) = (radius * r * a.cos(), radius * r * a.sin() * 0.32);
                let (px, py) = (cx + x * c - y * s, cy - (x * s + y * c));
                canvas.dot_over((px * 2.0).round() as i32, (py * 2.0).round() as i32, color, 0.9 - 0.25 * k as f32);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_moon(&self, canvas: &mut Canvas, cx: f32, cy: f32, r: f32, index: u8, angle: f32, color: Rgb) {
        let orbit = r * (2.3 + 0.9 * index as f32) + 1.0;
        let (x, y) = (cx + orbit * angle.cos(), cy - orbit * angle.sin() * 0.4);
        canvas.blend(x.floor() as i32, y.floor() as i32, color, 0.9);
    }

    fn draw_stars(&self, canvas: &mut Canvas, pal: &Palette) {
        let b = &self.bands;
        let dim = self.shrink();
        for star in &self.stars {
            let Some(born) = star.born else { continue };
            let (cx, cy) = ((star.fx * canvas.w as f32) as i32, (star.fy * canvas.h as f32) as i32);
            if canvas.covered(cx, cy) {
                continue;
            }
            let fade_in = smoothstep(0.0, 1.2, self.time - born);
            let twinkle = 0.5 + 0.5 * (star.phase + self.time * (0.8 + 2.6 * b.treble)).sin();
            let alpha = (fade_in * star.mag * (0.40 + 0.60 * twinkle) * (0.75 + 0.5 * b.intensity) * dim
                + star.flash)
                .clamp(0.0, 1.0);
            let glyph = if star.flash > 0.4 {
                '✦'
            } else if star.bright {
                if alpha > 0.7 { '✦' } else { '✧' }
            } else if alpha > 0.55 {
                '∙'
            } else {
                '·'
            };
            let color = match star.tint {
                0 => Rgb(0.84, 0.90, 1.0),
                1 => pal.petal_light,
                _ => pal.accent.mix(Rgb::WHITE, 0.3),
            };
            canvas.glyph(cx, cy, glyph, color, alpha);
        }
    }
}

fn normalize(v: (f32, f32, f32)) -> (f32, f32, f32) {
    let n = (v.0 * v.0 + v.1 * v.1 + v.2 * v.2).sqrt().max(1e-6);
    (v.0 / n, v.1 / n, v.2 / n)
}

/// Disco opaco con borde suavizado. `shade(dx, dy)` da el color de cada punto, con el
/// desplazamiento en píxeles desde el centro (`dy` hacia abajo).
fn disc(canvas: &mut Canvas, cx: f32, cy: f32, r: f32, shade: impl Fn(f32, f32) -> Rgb) {
    const SS: usize = 3;
    let (x0, x1) = ((cx - r - 1.0).floor() as i32, (cx + r + 1.0).ceil() as i32);
    let (y0, y1) = ((cy - r - 1.0).floor() as i32, (cy + r + 1.0).ceil() as i32);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (mut acc, mut hits) = ((0.0f32, 0.0f32, 0.0f32), 0u32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let dx = x as f32 + (sx as f32 + 0.5) / SS as f32 - cx;
                    let dy = y as f32 + (sy as f32 + 0.5) / SS as f32 - cy;
                    if dx * dx + dy * dy <= r * r {
                        let c = shade(dx, dy);
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
}

/// Suma luz en un disco. `falloff(distancia)` da la intensidad.
fn glow_disc(canvas: &mut Canvas, cx: f32, cy: f32, radius: f32, falloff: impl Fn(f32) -> f32, color: Rgb) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(bands: &Bands, progress: f32) -> Frame<'_> {
        Frame { dt: 1.0 / 60.0, bands, progress: Some(progress), song_len: 120.0, playing: true }
    }

    fn run(c: &mut Cosmos, from: f32, to: f32) {
        let bands = Bands { bass: 0.5, mid: 0.5, treble: 0.4, level: 0.5, intensity: 0.5, ..Bands::default() };
        let mut p = from;
        while p < to {
            c.update(&frame(&bands, p), 120, 40);
            p += 1.0 / (60.0 * 120.0);
        }
    }

    fn stars(c: &Cosmos) -> usize {
        c.stars.iter().filter(|s| s.born.is_some()).count()
    }

    #[test]
    fn order_is_stars_then_sun_then_planets() {
        let mut c = Cosmos::new(3);
        run(&mut c, 0.0, 0.03);
        assert!(stars(&c) > 10 && c.sun_level() == 0.0 && c.planets_born() == 0);
        run(&mut c, 0.03, 0.19);
        assert!(c.sun_level() > 0.9 && c.planets_born() == 0, "sol {} planetas {}", c.sun_level(), c.planets_born());
        assert!(stars(&c) > 150);
        run(&mut c, 0.19, 0.36);
        let mid = c.planets_born();
        assert!(mid >= 1 && mid < c.planets.len(), "a mitad: {mid} de {}", c.planets.len());
        run(&mut c, 0.36, 0.9);
        assert_eq!(c.planets_born(), c.planets.len());
    }

    #[test]
    fn systems_are_random_but_repeatable() {
        let (a, b, c) = (Cosmos::new(1), Cosmos::new(1), Cosmos::new(2));
        let shape = |c: &Cosmos| c.planets.iter().map(|p| (p.orbit, p.radius)).collect::<Vec<_>>();
        assert_eq!(shape(&a), shape(&b));
        assert_ne!(shape(&a), shape(&c));
        for seed in 0..40 {
            let n = Cosmos::new(seed).planets.len();
            assert!((4..=7).contains(&n));
        }
    }

    #[test]
    fn new_track_collapses_and_restarts() {
        let mut c = Cosmos::new(5);
        run(&mut c, 0.0, 0.9);
        let before: Vec<f32> = c.planets.iter().map(|p| p.orbit).collect();
        c.new_track();
        run(&mut c, 0.0, 0.03);
        assert!(c.collapse.is_none() && c.planets_born() == 0);
        let after: Vec<f32> = c.planets.iter().map(|p| p.orbit).collect();
        assert_ne!(before, after);
    }

    #[test]
    fn draws_at_any_size_without_panicking() {
        let mut c = Cosmos::new(9);
        run(&mut c, 0.0, 0.95);
        let pal = Palette::default();
        for (w, h) in [(0, 0), (1, 1), (3, 2), (40, 12), (200, 60)] {
            let mut canvas = Canvas::new(w, h);
            c.draw(&mut canvas, &pal);
        }
    }

    #[test]
    fn climax_sends_a_wave_and_comets() {
        let mut c = Cosmos::new(4);
        run(&mut c, 0.0, 0.6);
        let bands = Bands { climax: true, intensity: 1.0, level: 0.8, ..Bands::default() };
        c.update(&frame(&bands, 0.6), 120, 40);
        assert!(c.waves.iter().any(|w| w.strength > 1.0));
        assert!(c.comets.len() >= 7);
    }
}

//! Ambiente del jardín: lo que llena el espacio alrededor de las plantas.
//!
//! Cielo: aurora que sigue la intensidad de la canción, luna y estrellas.
//! Fondo: dos filas de cerros. Suelo: flores silvestres y juncos que brotan con el avance.
//! Aire: luciérnagas que destellan con los platillos y mariposas que rondan las flores.
//!
//! Todo depende del clima (`Atmosphere`), que sale del ánimo de la canción: sol y nubes
//! blancas si es alegre, atardecer si es serena, noche con luna si es nostálgica, lluvia si
//! es triste, tormenta con relámpagos si es intensa.

use std::f32::consts::TAU;

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::mood::{Atmosphere, Mood};
use crate::palette::Palette;
use crate::plant::{Rng, smoothstep};
use crate::render::View;

struct SkyStar {
    fx: f32,
    fy: f32,
    mag: f32,
    phase: f32,
}

struct Firefly {
    x: f32,
    y: f32,
    heading: f32,
    phase: f32,
    flash: f32,
    /// 0..1: aparece y desaparece suave.
    life: f32,
}

struct Butterfly {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    target: (f32, f32),
    /// Segundos hasta elegir otro destino.
    timer: f32,
    flap: f32,
    role: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WildKind {
    Flower,
    Reeds,
}

struct Wild {
    /// Posición como fracción del medio ancho visible, −1..1.
    fx: f32,
    height: f32,
    kind: WildKind,
    /// Avance de la canción en que brota.
    birth: f32,
    /// Segundos desde que brotó; negativo = aún no.
    age: f32,
    phase: f32,
    role: u8,
    variant: u8,
}

pub struct Scenery {
    rng: Rng,
    time: f32,
    /// Avance de la canción, 0..1.
    age: f32,
    bands: Bands,
    kick_pulse: f32,
    climax_pulse: f32,
    half_width: f32,
    stars: Vec<SkyStar>,
    fireflies: Vec<Firefly>,
    butterflies: Vec<Butterfly>,
    wild: Vec<Wild>,
    /// Fases de las ondas que forman los cerros.
    hills: [f32; 6],
    /// La luna va a la izquierda (−1) o a la derecha (+1).
    moon_side: f32,
    /// Desvanecimiento al cambiar de canción: 0..1.
    fading: Option<f32>,
    /// Clima actual: se acerca de a poco al que pide el ánimo.
    atm: Atmosphere,
    /// Ánimo fijado a mano; `None` = el de la canción.
    forced: Option<Mood>,
    /// Solo el cielo (océano, ciudad): sin cerros, flores ni bichos, y la lluvia cae hasta
    /// abajo.
    pub bare: bool,
    /// Nubes: (posición 0..1, altura 0..1, tamaño, forma).
    clouds: Vec<(f32, f32, f32, f32)>,
    /// Ráfaga de viento al cambiar de sección: 1 y decae.
    gust: f32,
    /// Relámpago: brillo 1 y decae, con el trazo del rayo.
    flash: f32,
    bolt: Vec<(f32, f32)>,
}

impl Scenery {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery {
            rng: Rng::new(seed ^ 0x5CE7_E121),
            time: 0.0,
            age: 0.0,
            bands: Bands::default(),
            kick_pulse: 0.0,
            climax_pulse: 0.0,
            half_width: 1.0,
            stars: Vec::new(),
            fireflies: Vec::new(),
            butterflies: Vec::new(),
            wild: Vec::new(),
            hills: [0.0; 6],
            moon_side: 1.0,
            fading: None,
            atm: Atmosphere::default(),
            forced: None,
            bare: false,
            clouds: Vec::new(),
            gust: 0.0,
            flash: 0.0,
            bolt: Vec::new(),
        };
        scenery.generate();
        scenery
    }

    fn generate(&mut self) {
        let rng = &mut self.rng;
        self.fading = None;
        self.fireflies.clear();
        self.butterflies.clear();
        self.moon_side = if rng.next() < 0.5 { -1.0 } else { 1.0 };
        for h in &mut self.hills {
            *h = rng.range(0.0, TAU);
        }
        self.clouds = (0..9)
            .map(|_| (rng.next(), rng.range(0.06, 0.42), rng.range(0.7, 1.5), rng.range(0.0, TAU)))
            .collect();
        self.stars = (0..64)
            .map(|_| SkyStar {
                fx: rng.next(),
                fy: rng.next() * 0.62,
                mag: 0.3 + 0.7 * rng.next().powf(2.0),
                phase: rng.range(0.0, TAU),
            })
            .collect();
        self.wild = (0..18)
            .map(|i| {
                let kind = if rng.next() < 0.55 { WildKind::Flower } else { WildKind::Reeds };
                Wild {
                    fx: rng.range(-0.97, 0.97),
                    height: match kind {
                        WildKind::Flower => rng.range(0.035, 0.085),
                        WildKind::Reeds => rng.range(0.05, 0.12),
                    },
                    kind,
                    birth: 0.03 + 0.55 * (i as f32 + rng.next()) / 18.0,
                    age: -1.0,
                    phase: rng.range(0.0, TAU),
                    role: (rng.next() * 3.0) as u8,
                    variant: (rng.next() * 4.0) as u8,
                }
            })
            .collect();
    }

    /// Fija el ánimo a mano (`None` = seguir el de la canción).
    pub fn force_mood(&mut self, mood: Option<Mood>) {
        self.forced = mood;
        if let Some(m) = mood {
            self.atm = Atmosphere::of(m);
        }
    }

    pub fn atmosphere(&self) -> &Atmosphere {
        &self.atm
    }

    /// Viento actual, 0..1 y más: el del clima más la ráfaga.
    pub fn wind(&self) -> f32 {
        self.atm.wind * (0.7 + 0.3 * (self.time * 0.6).sin()) + 0.6 * self.gust
    }

    /// Color del cielo a la altura `v` (0 arriba, 1 en el suelo).
    pub fn sky(&self, v: f32) -> Rgb {
        self.atm.sky_top.mix(self.atm.sky_bottom, v).add(Rgb::WHITE, 0.30 * self.flash * (1.0 - v))
    }

    /// Canción nueva: el ambiente se apaga y se rehace.
    pub fn fade(&mut self) {
        if self.fading.is_none() {
            self.fading = Some(0.0);
        }
    }

    /// 1 normal, 0 apagado del todo.
    fn presence(&self) -> f32 {
        self.fading.map(|t| 1.0 - smoothstep(0.0, 1.0, t)).unwrap_or(1.0)
    }

    /// `flowers`: centros de las flores del jardín, en coordenadas de mundo de la principal.
    pub fn update(
        &mut self,
        dt: f32,
        bands: &Bands,
        progress: Option<f32>,
        playing: bool,
        half_width: f32,
        flowers: &[(f32, f32)],
    ) {
        self.time += dt;
        self.bands = *bands;
        self.half_width = half_width.max(0.3);
        self.kick_pulse = (self.kick_pulse * (-dt / 0.2).exp()).max(if bands.kick { 1.0 } else { 0.0 });
        self.climax_pulse = (self.climax_pulse * (-dt / 2.5).exp()).max(if bands.climax { 1.0 } else { 0.0 });

        // Clima: sigue al ánimo de la canción, con calma. Mientras el ánimo aún no es
        // fiable (primeros segundos en Spotify) se queda cerca de la noche por defecto.
        let target = match self.forced {
            Some(mood) => Atmosphere::of(mood),
            None => {
                let mut blend = Atmosphere::default();
                blend.approach(&Atmosphere::for_mood(bands.song.valence, bands.song.arousal), bands.song.settled);
                blend
            }
        };
        self.atm.approach(&target, 1.0 - (-dt / 3.5).exp());
        self.gust = (self.gust * (-dt / 2.0).exp()).max(if bands.section { 1.0 } else { 0.0 });
        self.flash *= (-dt / 0.22).exp();
        // Relámpago: en los clímax y, de vez en cuando, en un bombo fuerte.
        let strike = bands.climax || (bands.kick && bands.bass > 0.7 && self.rng.next() < 0.05);
        if self.atm.storm > 0.4 && strike {
            self.flash = 1.0;
            let mut x = self.rng.range(0.15, 0.85);
            self.bolt = (0..9)
                .map(|i| {
                    x += self.rng.range(-0.035, 0.035);
                    (x, i as f32 / 8.0 * self.rng.range(0.55, 0.7))
                })
                .collect();
        }
        for cloud in &mut self.clouds {
            cloud.0 = (cloud.0 + dt * (0.004 + 0.03 * self.atm.wind + 0.02 * self.gust)).rem_euclid(1.3);
        }

        if let Some(t) = &mut self.fading {
            *t += dt / 1.2;
            if *t >= 1.0 {
                self.generate();
                self.age = 0.0;
            }
            return;
        }
        if let Some(p) = progress {
            self.age = p;
        } else if playing {
            self.age = (self.age + dt / 210.0).min(1.0);
        }

        let age = self.age;
        for w in &mut self.wild {
            if w.age < 0.0 && age >= w.birth {
                w.age = 0.0;
            } else if w.age >= 0.0 {
                w.age += dt;
            }
        }
        self.update_fireflies(dt, bands);
        self.update_butterflies(dt, bands, flowers);
    }

    fn update_fireflies(&mut self, dt: f32, bands: &Bands) {
        let wanted =
            ((3.0 + 9.0 * smoothstep(0.03, 0.5, self.age) + 4.0 * bands.intensity) * self.atm.fireflies) as usize;
        if self.fireflies.len() < wanted && self.rng.next() < dt * 1.5 {
            let hw = self.half_width;
            self.fireflies.push(Firefly {
                x: self.rng.range(-hw, hw),
                y: self.rng.range(0.05, 0.55),
                heading: self.rng.range(0.0, TAU),
                phase: self.rng.range(0.0, TAU),
                flash: 0.0,
                life: 0.0,
            });
        }
        // Cada platillo hace destellar a una.
        if bands.hat && !self.fireflies.is_empty() {
            let i = (self.rng.next() * self.fireflies.len() as f32) as usize % self.fireflies.len();
            self.fireflies[i].flash = 1.0;
        }
        let (hw, time, extra) = (self.half_width, self.time, self.fireflies.len() > wanted + 2);
        for (i, f) in self.fireflies.iter_mut().enumerate() {
            // Vuelo errante: el rumbo cambia suave y se corrige cerca de los bordes.
            f.heading += ((time * 0.7 + f.phase).sin() + (time * 1.9 + f.phase * 2.0).sin() * 0.6) * dt * 1.4;
            let speed = 0.035 + 0.03 * bands.mid;
            f.x += f.heading.cos() * speed * dt;
            f.y += f.heading.sin() * speed * 0.6 * dt;
            if f.x.abs() > hw * 0.97 || !(0.04..0.6).contains(&f.y) {
                f.heading = (0.3 - f.y).atan2(-f.x);
            }
            f.flash *= (-dt / 0.35).exp();
            // Si sobran, la primera se va apagando.
            let step = if extra && i == 0 { -dt } else { dt };
            f.life = (f.life + step / 1.5).clamp(0.0, 1.0);
        }
        self.fireflies.retain(|f| !(extra && f.life <= 0.0));
    }

    fn update_butterflies(&mut self, dt: f32, bands: &Bands, flowers: &[(f32, f32)]) {
        // Llegan cuando las flores ya abrieron.
        let wanted = (4.0 * smoothstep(0.18, 0.45, self.age) * self.atm.butterflies).round() as usize;
        if self.butterflies.len() < wanted && self.rng.next() < dt * 0.6 {
            let side = if self.rng.next() < 0.5 { -1.0 } else { 1.0 };
            self.butterflies.push(Butterfly {
                x: side * (self.half_width + 0.08),
                y: self.rng.range(0.25, 0.6),
                vx: 0.0,
                vy: 0.0,
                target: (0.0, 0.5),
                timer: 0.0,
                flap: self.rng.range(0.0, TAU),
                role: (self.rng.next() * 2.0) as u8,
            });
        }
        for i in 0..self.butterflies.len() {
            let pick_flower = !flowers.is_empty() && self.rng.next() < 0.75;
            let jitter = (self.rng.range(-0.09, 0.09), self.rng.range(0.02, 0.12));
            let roam = (self.rng.range(-self.half_width, self.half_width) * 0.85, self.rng.range(0.2, 0.75));
            let flower = flowers
                .get((self.rng.next() * flowers.len() as f32) as usize % flowers.len().max(1))
                .copied();
            let pause = self.rng.range(3.0, 7.0);
            let b = &mut self.butterflies[i];
            b.timer -= dt;
            if b.timer <= 0.0 {
                b.timer = pause;
                b.target = match (pick_flower, flower) {
                    (true, Some((fx, fy))) => (fx + jitter.0, fy + jitter.1),
                    _ => roam,
                };
            }
            // Va hacia el destino con inercia y un aleteo que sube y baja.
            let (dx, dy) = (b.target.0 - b.x, b.target.1 - b.y);
            let dist = (dx * dx + dy * dy).sqrt().max(1e-3);
            let cruise = (0.13 + 0.08 * bands.mid) * (dist / 0.15).min(1.0);
            b.vx += (dx / dist * cruise - b.vx) * (1.0 - (-dt * 1.6).exp());
            b.vy += (dy / dist * cruise - b.vy) * (1.0 - (-dt * 1.6).exp());
            b.flap += dt * (9.0 + 8.0 * bands.mid);
            b.x += b.vx * dt;
            b.y += (b.vy + b.flap.sin() * 0.02) * dt;
        }
    }

    /// Altura de los cerros sobre el suelo, como fracción de la altura del cielo.
    /// `layer` 0 = lejanos, 1 = cercanos. `u` = posición horizontal 0..1.
    fn hill(&self, layer: usize, u: f32) -> f32 {
        let p = &self.hills[layer * 3..layer * 3 + 3];
        match layer {
            0 => {
                0.20 + 0.07 * (u * TAU * 1.3 + p[0]).sin()
                    + 0.035 * (u * TAU * 3.1 + p[1]).sin()
                    + 0.015 * (u * TAU * 7.0 + p[2]).sin()
            }
            _ => {
                0.085 + 0.04 * (u * TAU * 2.2 + p[0]).sin()
                    + 0.02 * (u * TAU * 5.3 + p[1]).sin()
                    + 0.008 * (u * TAU * 11.0 + p[2]).sin()
            }
        }
    }

    /// Fila de píxel del horizonte (el cerro más alto) en la columna `x`.
    fn skyline(&self, view: &View, x: f32, w: f32) -> f32 {
        if self.bare {
            return view.ground;
        }
        let u = x / w.max(1.0);
        view.ground * (1.0 - self.hill(0, u).max(self.hill(1, u)))
    }

    fn moon(&self, canvas: &Canvas, view: &View) -> (f32, f32, f32) {
        let x = canvas.w as f32 * (0.5 + 0.34 * self.moon_side);
        (x, view.ground * 0.17, (view.ground * 0.065).clamp(2.5, 9.0))
    }

    /// Luces del cielo, para reflejarlas en el agua: (x, radio, color, intensidad) del sol
    /// y de la luna.
    pub fn lights(&self, canvas: &Canvas, view: &View) -> [(f32, f32, Rgb, f32); 2] {
        let k = self.presence();
        let height = self.atm.sun_height;
        let sun_x = canvas.w as f32 * (0.5 - 0.30 * self.moon_side);
        let sun_r = (view.ground * 0.075 * (1.35 - 0.45 * height)).clamp(2.5, 14.0);
        let sun_color = Rgb(1.0, 0.58, 0.26).mix(Rgb(1.0, 0.95, 0.74), smoothstep(0.2, 0.7, height));
        let (moon_x, _, moon_r) = self.moon(canvas, view);
        [
            (sun_x, sun_r, sun_color, k * self.atm.sun),
            (moon_x, moon_r, Rgb(0.82, 0.86, 1.0), k * self.atm.moon * 0.55),
        ]
    }

    /// Cielo y fondo: va antes que las plantas.
    pub fn draw_back(&self, canvas: &mut Canvas, view: &View, pal: &Palette) {
        let k = self.presence();
        self.draw_sun(canvas, view, k);
        self.draw_aurora(canvas, view, pal, k * self.atm.aurora);
        self.draw_moon(canvas, view, pal, k * self.atm.moon);
        self.draw_clouds(canvas, view, k);
        if !self.bare {
            self.draw_hills(canvas, view, pal, k);
        }
    }

    /// Cortinas de luz en el cielo. Casi invisibles en los pasajes suaves; se encienden con
    /// la intensidad y estallan en los clímax.
    fn draw_aurora(&self, canvas: &mut Canvas, view: &View, pal: &Palette, k: f32) {
        let b = &self.bands;
        let power = k * (0.010 + 0.10 * b.intensity * smoothstep(0.02, 0.2, self.age) + 0.20 * self.climax_pulse);
        if power < 0.006 {
            return;
        }
        let (w, sky, t) = (canvas.w, view.ground.max(1.0), self.time);
        for x in 0..w {
            let u = x as f32;
            // Dos cintas que ondulan; los medios les agregan un rizado rápido.
            let ripple = (u * 0.5 + t * 3.0).sin() * 0.012 * b.mid;
            let centers = [
                0.20 + 0.09 * (u * 0.045 + t * 0.20).sin() + 0.04 * (u * 0.11 - t * 0.33).sin() + ripple,
                0.38 + 0.07 * (u * 0.060 - t * 0.16).sin() + 0.03 * (u * 0.15 + t * 0.41).sin() - ripple,
            ];
            // Rayas verticales que se desplazan: la textura de cortina.
            let streak = 0.78 + 0.22 * (u * 0.9 + (u * 0.13 + t * 0.5).sin() * 3.0).sin();
            for y in 0..(sky * 0.75) as usize {
                let v = y as f32 / sky;
                let mut light = 0.0;
                let mut tone = 0.0;
                for (i, c) in centers.iter().enumerate() {
                    let d = v - c;
                    // Borde inferior nítido, cola larga hacia arriba.
                    let fall = if d > 0.0 { (-(d / 0.035).powi(2)).exp() } else { (-(d / 0.16).powi(2)).exp() };
                    light += fall * if i == 0 { 1.0 } else { 0.7 };
                    tone += fall * (-d / 0.16).clamp(0.0, 1.0);
                }
                let amount = power * light * streak;
                if amount > 0.004 {
                    let color = pal.accent.mix(pal.petal_light, (tone / light.max(1e-3)).clamp(0.0, 1.0));
                    canvas.glow(x as i32, y as i32, color, amount);
                }
            }
        }
    }

    /// Sol: disco con halo y rayos. Alto y pálido de día; bajo, grande y naranja al atardecer.
    fn draw_sun(&self, canvas: &mut Canvas, view: &View, k: f32) {
        let amount = k * self.atm.sun;
        if amount < 0.02 {
            return;
        }
        let height = self.atm.sun_height;
        let x = canvas.w as f32 * (0.5 - 0.30 * self.moon_side);
        let y = view.ground * (0.90 - 0.76 * height);
        let pulse = 1.0 + 0.05 * self.bands.level + 0.05 * self.kick_pulse;
        let r = (view.ground * 0.075 * (1.35 - 0.45 * height)).clamp(2.5, 14.0) * pulse;
        let color = Rgb(1.0, 0.58, 0.26).mix(Rgb(1.0, 0.95, 0.74), smoothstep(0.2, 0.7, height));
        let power = amount * (0.30 + 0.20 * self.bands.level + 0.25 * self.climax_pulse);
        let reach = r * 5.0;
        for py in (y - reach) as i32..=(y + reach) as i32 {
            for px in (x - reach) as i32..=(x + reach) as i32 {
                let (dx, dy) = (px as f32 + 0.5 - x, py as f32 + 0.5 - y);
                let d = (dx * dx + dy * dy).sqrt();
                let theta = dy.atan2(dx);
                let rays = 1.0 + 0.30 * (theta * 8.0 + self.time * 0.35).sin() * (0.5 + self.bands.level);
                let glow = power * (-(d - r).max(0.0) / (r * 1.3)).exp() * rays;
                if glow > 0.004 {
                    canvas.glow(px, py, color, glow);
                }
                // Borde suavizado en un píxel.
                let cover = (r - d + 0.5).clamp(0.0, 1.0);
                if cover > 0.0 {
                    canvas.paint(px, py, color.mix(Rgb::WHITE, 0.45 * (1.0 - d / r).max(0.0)), cover * amount);
                }
            }
        }
    }

    /// Nubes: grupos de manchas redondas que el viento arrastra. Con mal tiempo, además, un
    /// techo gris cubre la parte alta del cielo.
    fn draw_clouds(&self, canvas: &mut Canvas, view: &View, k: f32) {
        let amount = k * self.atm.clouds;
        if amount < 0.03 {
            return;
        }
        let (w, sky) = (canvas.w as f32, view.ground.max(1.0));
        let tone = self.atm.cloud_tone.add(Rgb::WHITE, 0.25 * self.flash);
        let overcast = smoothstep(0.55, 1.0, self.atm.clouds) * k;
        if overcast > 0.02 {
            for y in 0..(sky * 0.5) as i32 {
                let fade = 1.0 - y as f32 / (sky * 0.5);
                for x in 0..canvas.w as i32 {
                    let lump = 0.75 + 0.25 * (x as f32 * 0.09 + self.time * 0.05 * (1.0 + 3.0 * self.atm.wind)).sin();
                    canvas.paint(x, y, tone, 0.75 * overcast * fade * lump);
                }
            }
        }
        let shown = (self.clouds.len() as f32 * amount.min(1.0)).ceil() as usize;
        for (fx, fy, size, shape) in self.clouds.iter().take(shown) {
            let (cx, cy) = ((fx - 0.15) * w, fy * sky);
            let unit = (sky * 0.055 * size).max(1.5);
            // Tres o cuatro lóbulos por nube.
            for i in 0..4 {
                let a = shape + i as f32 * 1.7;
                let (ox, oy) = ((i as f32 - 1.5) * unit * 1.25, a.sin() * unit * 0.3);
                let (rx, ry) = (unit * (1.5 + 0.4 * a.cos()), unit * (0.75 + 0.2 * a.sin()));
                for py in (cy + oy - ry) as i32..=(cy + oy + ry) as i32 {
                    for px in (cx + ox - rx) as i32..=(cx + ox + rx) as i32 {
                        let (dx, dy) = ((px as f32 + 0.5 - cx - ox) / rx, (py as f32 + 0.5 - cy - oy) / ry);
                        let d = dx * dx + dy * dy;
                        if d < 1.0 {
                            // Más clara arriba, como iluminada desde el cielo.
                            let lit = tone.add(Rgb::WHITE, 0.10 * (-dy).max(0.0));
                            canvas.paint(px, py, lit, (1.0 - d).powf(0.6) * 0.85 * amount.min(1.0));
                        }
                    }
                }
            }
        }
    }

    /// Lluvia: trazos inclinados por el viento. Arrecia en los pasajes intensos.
    fn draw_rain(&self, canvas: &mut Canvas, view: &View, k: f32) {
        let amount = k * self.atm.rain;
        if amount < 0.03 {
            return;
        }
        let floor = if self.bare { canvas.px_h() as f32 } else { view.ground };
        let (w, sky) = (canvas.w as f32 * 2.0, floor * 2.0);
        let drops = (canvas.w as f32 * 1.6 * amount * (0.5 + 0.9 * self.bands.intensity)) as u32;
        let slant = 0.25 + 0.9 * self.wind();
        let color = self.atm.sky_bottom.mix(Rgb(0.70, 0.80, 0.95), 0.65);
        for i in 0..drops {
            // Posición pseudoaleatoria fija por gota; el tiempo solo la hace caer.
            let seed = i.wrapping_mul(2654435761);
            let (a, b) = ((seed >> 8) as f32 / (1 << 24) as f32, (seed.wrapping_mul(40503) >> 8) as f32 / (1 << 24) as f32);
            let speed = 1.1 + 0.6 * b;
            let fall = (b + self.time * speed).fract();
            let y = fall * sky;
            let x = (a * (w + sky * slant) - sky * slant + y * slant).rem_euclid(w);
            let length = 3 + (b * 3.0) as i32;
            for j in 0..length {
                let (px, py) = (x - j as f32 * slant, y - j as f32);
                canvas.dot_over(px as i32, py as i32, color, 0.55 * amount.min(1.0) * (1.0 - j as f32 / length as f32));
            }
        }
    }

    /// Rayo: una línea quebrada desde las nubes, mientras dura el destello.
    fn draw_bolt(&self, canvas: &mut Canvas, view: &View) {
        if self.flash < 0.25 || self.bolt.len() < 2 {
            return;
        }
        let (w, sky) = (canvas.w as f32 * 2.0, view.ground * 2.0);
        let color = Rgb(0.95, 0.95, 1.0);
        for pair in self.bolt.windows(2) {
            let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
            canvas.line(x0 * w, y0 * sky, x1 * w, y1 * sky, color);
            canvas.line(x0 * w + 1.0, y0 * sky, x1 * w + 1.0, y1 * sky, color);
        }
    }

    /// Luna creciente con un halo que respira con el volumen.
    fn draw_moon(&self, canvas: &mut Canvas, view: &View, pal: &Palette, k: f32) {
        let (mx, my, r) = self.moon(canvas, view);
        let color = pal.petal_light.mix(Rgb::WHITE, 0.6);
        let halo = k * (0.07 + 0.10 * self.bands.level + 0.10 * self.climax_pulse);
        let reach = r * 4.0;
        for y in (my - reach) as i32..=(my + reach) as i32 {
            for x in (mx - reach) as i32..=(mx + reach) as i32 {
                let d = ((x as f32 + 0.5 - mx).powi(2) + (y as f32 + 0.5 - my).powi(2)).sqrt();
                let glow = halo * (-((d - r).max(0.0) / (r * 1.3)).powi(2)).exp();
                if glow > 0.004 {
                    canvas.glow(x, y, color, glow);
                }
                // Disco menos otro disco corrido: queda la medialuna.
                const SS: i32 = 3;
                let mut lit = 0;
                for sy in 0..SS {
                    for sx in 0..SS {
                        let px = x as f32 + (sx as f32 + 0.5) / SS as f32 - mx;
                        let py = y as f32 + (sy as f32 + 0.5) / SS as f32 - my;
                        let inside = px * px + py * py <= r * r;
                        let (cx, cy) = (px - 0.5 * r * self.moon_side, py + 0.18 * r);
                        let shadow = cx * cx + cy * cy <= (0.92 * r).powi(2);
                        lit += (inside && !shadow) as i32;
                    }
                }
                if lit > 0 {
                    canvas.paint(x, y, color, k * lit as f32 / (SS * SS) as f32);
                }
            }
        }
    }

    fn draw_hills(&self, canvas: &mut Canvas, view: &View, pal: &Palette, k: f32) {
        let (w, ground) = (canvas.w, view.ground);
        let sky = self.atm.sky_top.mix(self.atm.sky_bottom, 0.8);
        // De noche los cerros apenas se distinguen del cielo; de día se ven verdes.
        let day = self.atm.daylight;
        for (layer, night, noon) in [(0usize, 0.11f32, 0.42f32), (1, 0.20, 0.62)] {
            let body = sky.mix(pal.stem.mix(pal.leaf, 0.5 * day), night + (noon - night) * day);
            // El borde de los cerros lejanos se ilumina con los graves.
            let rim = body.add(pal.accent, if layer == 0 { 0.05 + 0.12 * self.bands.bass } else { 0.03 });
            for x in 0..w {
                let top = ground * (1.0 - self.hill(layer, x as f32 / w.max(1) as f32));
                let first = top.floor() as i32;
                for y in first.max(0)..ground as i32 {
                    // Cobertura parcial en el borde, para suavizar el perfil.
                    let cover = if y == first { 1.0 - top.fract() } else { 1.0 };
                    let color = if y <= first + 1 { rim } else { body };
                    canvas.paint(x as i32, y, color, k * cover * 0.92);
                }
            }
        }
    }

    /// Flores silvestres y juncos: van junto con el pasto, antes de la tierra.
    /// `stems`: posición horizontal de los tallos del jardín, para no brotar encima.
    pub fn draw_ground(&self, canvas: &mut Canvas, view: &View, pal: &Palette, stems: &[f32]) {
        let k = self.presence();
        let bass = self.bands.bass;
        let half_width = view.half_width();
        for w in self.wild.iter().filter(|w| w.age >= 0.0) {
            let x = w.fx * half_width;
            if stems.iter().any(|s| (x - s).abs() < 0.07) {
                continue;
            }
            let grow = (1.0 - (1.0 - (w.age / 2.0).clamp(0.0, 1.0)).powi(3)) * k;
            let height = w.height * grow * (1.0 + 0.10 * self.kick_pulse);
            if height * view.scale < 1.0 {
                continue;
            }
            let sway = (self.time * 1.5 + w.phase).sin() * (0.35 + bass);
            match w.kind {
                WildKind::Flower => {
                    let lean = 0.014 * sway;
                    let tip = self.blade(canvas, view, x, height, lean, pal.leaf.mix(pal.stem, 0.4));
                    let color = [pal.petal, pal.accent, pal.petal_light][w.role as usize % 3];
                    let glyph = if grow < 0.7 { '•' } else { ['✿', '❀', '❁', '✾'][w.variant as usize % 4] };
                    let shine = 0.72 + 0.28 * self.kick_pulse;
                    canvas.glyph(tip.0.floor() as i32, (tip.1 / 2.0).floor() as i32, glyph, color, shine * k);
                }
                WildKind::Reeds => {
                    let blades = 3 + w.variant as usize % 3;
                    for i in 0..blades {
                        let spread = (i as f32 - (blades - 1) as f32 / 2.0) * 0.012;
                        let lean = spread * 1.6 + 0.02 * sway;
                        let h = height * (0.65 + 0.35 * ((i * 7 + w.variant as usize) % 3) as f32 / 2.0);
                        self.blade(canvas, view, x + spread, h, lean, pal.leaf.mix(pal.stem, 0.25 + 0.2 * i as f32 / 4.0));
                    }
                }
            }
        }
    }

    /// Tallo curvo desde el suelo. Devuelve la punta, en píxeles.
    fn blade(&self, canvas: &mut Canvas, view: &View, x: f32, height: f32, lean: f32, color: Rgb) -> (f32, f32) {
        const SEGMENTS: usize = 6;
        let mut prev = view.px(x, 0.0);
        for i in 1..=SEGMENTS {
            let u = i as f32 / SEGMENTS as f32;
            let cur = view.px(x + lean * u * u, height * u);
            canvas.line(prev.0 * 2.0, prev.1 * 2.0, cur.0 * 2.0, cur.1 * 2.0, color);
            prev = cur;
        }
        prev
    }

    /// Estrellas, luciérnagas y mariposas: van al final, sobre todo lo demás.
    pub fn draw_front(&self, canvas: &mut Canvas, view: &View, pal: &Palette) {
        let k = self.presence();
        let b = &self.bands;
        self.draw_rain(canvas, view, k);
        self.draw_bolt(canvas, view);

        // Estrellas: solo sobre el horizonte y donde no hay nada dibujado.
        let w = canvas.w as f32;
        let moon_r = self.moon(canvas, view);
        let visible = 8.0 + self.stars.len() as f32 * smoothstep(0.0, 0.3, self.age);
        let k_stars = k * self.atm.stars;
        for (i, star) in self.stars.iter().enumerate() {
            if i as f32 > visible {
                break;
            }
            let (px, py) = (star.fx * w, star.fy * view.ground);
            let (cx, cy) = (px as i32, (py / 2.0) as i32);
            let near_moon = (px - moon_r.0).hypot(py - moon_r.1) < moon_r.2 * 1.6;
            if py > self.skyline(view, px, w) - 2.0 || near_moon || canvas.occupied(cx, cy) {
                continue;
            }
            let twinkle = 0.5 + 0.5 * (star.phase + self.time * (0.7 + 2.4 * b.treble)).sin();
            let alpha = k_stars * star.mag * (0.30 + 0.55 * twinkle) * (0.8 + 0.4 * b.intensity);
            if alpha < 0.03 {
                continue;
            }
            let glyph = if star.mag > 0.85 && twinkle > 0.6 { '✧' } else if alpha > 0.5 { '∙' } else { '·' };
            canvas.glyph(cx, cy, glyph, pal.petal_light.mix(Rgb::WHITE, 0.5), alpha.clamp(0.0, 1.0));
        }

        if self.bare {
            return;
        }

        // Luciérnagas: un punto con halo que late.
        let light = pal.accent.mix(Rgb::WHITE, 0.35);
        for f in &self.fireflies {
            let blink = 0.30 + 0.70 * (f.phase + self.time * 1.3).sin().max(0.0).powi(3);
            let bright = (blink + f.flash).min(1.6) * f.life * k;
            let (px, py) = view.px(f.x, f.y);
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let d2 = (dx * dx + dy * dy) as f32;
                    canvas.glow(px as i32 + dx, py as i32 + dy, light, 0.20 * bright * (-d2 / 2.2).exp());
                }
            }
            let (cx, cy) = (px.floor() as i32, (py / 2.0).floor() as i32);
            if !canvas.occupied(cx, cy) {
                canvas.glyph(cx, cy, if bright > 0.8 { '•' } else { '·' }, light, bright.min(1.0));
            }
        }

        // Mariposas: cuerpo y dos alas que se abren y cierran.
        for bf in &self.butterflies {
            let (px, py) = view.px(bf.x, bf.y);
            let (bx, by) = ((px * 2.0).round() as i32, (py * 2.0).round() as i32);
            let wing = [pal.accent, pal.petal_light][bf.role as usize % 2].mix(Rgb::WHITE, 0.15);
            let span = ((0.020 * view.scale * 2.0).max(2.0) * bf.flap.sin().abs()).round() as i32;
            for side in [-1, 1] {
                for i in 1..=span {
                    // Ala superior grande, inferior chica.
                    for j in -(i + 1) / 2 - 1..=i / 3 {
                        canvas.dot_over(bx + side * i, by + j, wing, k);
                    }
                }
            }
            canvas.dot_over(bx, by - 1, wing.scale(0.55), k);
            canvas.dot_over(bx, by, wing.scale(0.55), k);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bands() -> Bands {
        Bands { bass: 0.5, mid: 0.5, treble: 0.4, level: 0.5, intensity: 0.6, ..Bands::default() }
    }

    fn run(s: &mut Scenery, from: f32, to: f32) {
        let flowers = [(0.0, 0.6), (-0.4, 0.3)];
        let mut p = from;
        while p < to {
            s.update(1.0 / 60.0, &bands(), Some(p), true, 1.2, &flowers);
            p += 1.0 / (60.0 * 120.0);
        }
    }

    #[test]
    fn ambience_fills_in_with_the_song() {
        let mut s = Scenery::new(1);
        run(&mut s, 0.0, 0.02);
        let early = s.wild.iter().filter(|w| w.age >= 0.0).count();
        assert!(early <= 1 && s.butterflies.is_empty());
        run(&mut s, 0.02, 0.7);
        assert_eq!(s.wild.iter().filter(|w| w.age >= 0.0).count(), s.wild.len());
        assert!(s.fireflies.len() >= 8, "luciérnagas: {}", s.fireflies.len());
        assert!((1..=3).contains(&s.butterflies.len()), "mariposas: {}", s.butterflies.len());
    }

    #[test]
    fn creatures_stay_on_screen() {
        let mut s = Scenery::new(2);
        run(&mut s, 0.0, 0.9);
        for f in &s.fireflies {
            assert!(f.x.abs() <= 1.3 && (0.0..0.7).contains(&f.y), "luciérnaga en ({}, {})", f.x, f.y);
        }
        for b in &s.butterflies {
            assert!(b.x.abs() <= 1.5 && (0.0..1.0).contains(&b.y), "mariposa en ({}, {})", b.x, b.y);
        }
    }

    #[test]
    fn hills_stay_below_a_third_of_the_sky() {
        for seed in 0..20 {
            let s = Scenery::new(seed);
            for i in 0..=100 {
                let u = i as f32 / 100.0;
                assert!((0.0..0.36).contains(&s.hill(0, u)) && (0.0..0.2).contains(&s.hill(1, u)));
            }
        }
    }

    #[test]
    fn fade_regenerates() {
        let mut s = Scenery::new(3);
        run(&mut s, 0.0, 0.8);
        let before: Vec<f32> = s.wild.iter().map(|w| w.fx).collect();
        s.fade();
        run(&mut s, 0.0, 0.02);
        assert!(s.fading.is_none());
        assert_ne!(before, s.wild.iter().map(|w| w.fx).collect::<Vec<_>>());
    }

    #[test]
    fn draws_at_any_size_without_panicking() {
        let mut s = Scenery::new(4);
        run(&mut s, 0.0, 0.9);
        let pal = Palette::default();
        for (w, h) in [(0, 0), (1, 1), (5, 3), (40, 12), (220, 60)] {
            let mut canvas = Canvas::new(w, h);
            canvas.clear(|_, _| Rgb::BLACK);
            let view = View::new(&canvas, 0.66);
            s.draw_back(&mut canvas, &view, &pal);
            s.draw_ground(&mut canvas, &view, &pal, &[0.0]);
            s.draw_front(&mut canvas, &view, &pal);
        }
    }
}

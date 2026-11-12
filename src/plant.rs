//! Simulación de la planta: tallo, hojas, zarcillos, flor y partículas.
//!
//! Coordenadas de mundo: origen en la base del tallo, `y` hacia arriba. Una unidad es la
//! altura útil de la pantalla, así que la planta se adapta sola al tamaño de la terminal.

use std::f32::consts::{PI, TAU};

use crate::analysis::Bands;
use crate::features::Song;

/// Altura del tallo completo, en unidades de mundo.
pub const STEM_HEIGHT: f32 = 0.66;
/// Radio de la flor abierta.
pub const FLOWER_RADIUS: f32 = 0.19;
/// Fracción de la canción en la que el tallo llega arriba.
pub const GROW_END: f32 = 0.58;
/// Curva del crecimiento: 1 = parejo; más alto = más rápido al inicio.
pub const GROW_EASE: f32 = 1.35;
/// Largo base de una hoja.
pub const LEAF_LENGTH: f32 = 0.17;

const MAX_PARTICLES: usize = 420;
const MAX_LEAVES: usize = 12;

pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Generador xorshift: aleatorio reproducible sin dependencias.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

/// Rasgos de la flor que dependen de la canción: su huella. La misma canción da siempre
/// la misma flor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Genes {
    /// Número de pétalos: sale del tempo. Lenta = pocos, rápida = muchos.
    pub petals: u32,
    /// Forma del pétalo: −1 redondo (mayor), +1 puntiagudo (menor).
    pub sharp: f32,
    /// Capas de pétalos: 2 si la canción es calmada, 3 si es enérgica.
    pub layers: u8,
    /// Tamaño relativo: crece con la energía.
    pub size: f32,
    /// Corrimiento de tono sobre el color de la carátula: sale de la tónica.
    pub hue: f32,
}

impl Genes {
    pub fn from_song(song: &Song) -> Genes {
        let bpm = if song.bpm > 0.0 { song.bpm.clamp(60.0, 179.0) } else { 110.0 };
        Genes {
            // 60–84 BPM: 5 pétalos; 84–108: 6; 108–132: 7; 132–156: 8; más: 9.
            petals: 5 + ((bpm - 60.0) / 24.0) as u32,
            sharp: (-song.majorness).clamp(-1.0, 1.0),
            layers: if song.arousal > 0.5 { 3 } else { 2 },
            size: 0.88 + 0.24 * song.arousal,
            hue: (song.key as f32 / 12.0 - 0.5) * 0.10,
        }
    }

    /// Variante para las flores laterales: misma familia, otra forma.
    pub fn sibling(&self, delta: i32) -> Genes {
        Genes { petals: (self.petals as i32 + delta).clamp(4, 11) as u32, ..*self }
    }
}

#[derive(Clone, Copy)]
pub struct Leaf {
    /// Posición a lo largo del tallo (0 = base, 1 = punta).
    pub s: f32,
    /// −1 izquierda, +1 derecha.
    pub side: f32,
    pub size: f32,
    /// Ángulo sobre la horizontal, en radianes.
    pub angle: f32,
    pub age: f32,
    /// Variación de tono propia de la hoja, 0..1.
    pub tint: f32,
}

#[derive(Clone, Copy)]
pub struct Tendril {
    pub s: f32,
    pub side: f32,
    pub size: f32,
    pub age: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ParticleKind {
    /// Polen: sale del centro con los agudos.
    Pollen,
    /// Chispa: estallido en cada golpe de platillo.
    Spark,
    /// Pétalo que cae en algunos golpes de bombo.
    Petal,
    /// Mota de fondo, flota lento.
    Mote,
}

#[derive(Clone, Copy)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub age: f32,
    pub life: f32,
    pub kind: ParticleKind,
    pub phase: f32,
    pub variant: u8,
}

/// Geometría instantánea de la flor, en unidades de mundo.
#[derive(Clone, Copy)]
pub struct Flower {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    /// 0 = capullo cerrado, 1 = abierta.
    pub open: f32,
    pub rot: f32,
    pub petals: u32,
    /// Rasgos fijados por la canción, si ya se conocen.
    pub genes: Option<Genes>,
}

pub struct Plant {
    pub time: f32,
    /// Cuánto del tallo ha crecido, 0..1.
    pub growth: f32,
    /// Apertura acumulada de la flor, 0..1.
    pub open: f32,
    pub bands: Bands,
    /// Sube a 1 en cada bombo y decae.
    pub kick_pulse: f32,
    /// Sube a 1 en cada platillo y decae.
    pub hat_pulse: f32,
    /// Sube a 1 al empezar un clímax y decae en un par de segundos.
    pub climax_pulse: f32,
    /// Sube a 1 en cada pulso previsto y decae.
    pub beat_pulse: f32,
    /// Reloj del vaivén del tallo: avanza al ritmo de la canción cuando se conoce el tempo.
    sway_clock: f32,
    /// Viento de la escena: exagera el vaivén.
    pub wind: f32,
    /// Pulsos de luz que suben por el tallo (posición `s`).
    pub pulses: Vec<f32>,
    pub rot: f32,
    pub petals: u32,
    pub leaves: Vec<Leaf>,
    pub tendrils: Vec<Tendril>,
    pub particles: Vec<Particle>,
    /// La planta se está marchitando (cambio de canción).
    pub withering: bool,
    /// Huella de la canción en la flor. `None` hasta que el análisis sea fiable.
    pub genes: Option<Genes>,
    /// Emite motas de fondo. Solo la planta principal.
    pub ambient: bool,
    phase: [f32; 3],
    rng: Rng,
    last_leaf_s: f32,
    leaf_side: f32,
    pollen_debt: f32,
    fixed_petals: Option<u32>,
}

impl Plant {
    /// `petals`: número fijo de pétalos, o `None` para que dependa de la semilla.
    pub fn new(seed: u64, petals: Option<u32>) -> Self {
        let mut plant = Plant {
            time: 0.0,
            growth: 0.0,
            open: 0.0,
            bands: Bands::default(),
            kick_pulse: 0.0,
            hat_pulse: 0.0,
            climax_pulse: 0.0,
            beat_pulse: 0.0,
            sway_clock: 0.0,
            wind: 0.0,
            pulses: Vec::new(),
            rot: 0.0,
            petals: 6,
            leaves: Vec::new(),
            tendrils: Vec::new(),
            particles: Vec::new(),
            withering: false,
            genes: None,
            ambient: true,
            phase: [0.0; 3],
            rng: Rng::new(seed),
            last_leaf_s: 0.0,
            leaf_side: 1.0,
            pollen_debt: 0.0,
            fixed_petals: petals,
        };
        plant.reseed(seed);
        plant
    }

    /// Reinicia la planta con otra forma. Las partículas en vuelo se conservan.
    pub fn reseed(&mut self, seed: u64) {
        self.rng = Rng::new(seed);
        self.growth = 0.0;
        self.open = 0.0;
        self.withering = false;
        self.genes = None;
        self.leaves.clear();
        self.tendrils.clear();
        self.pulses.clear();
        self.last_leaf_s = 0.0;
        self.leaf_side = if self.rng.next() < 0.5 { -1.0 } else { 1.0 };
        self.phase = [self.rng.range(0.0, TAU), self.rng.range(0.0, TAU), self.rng.range(0.0, TAU)];
        self.petals = self
            .fixed_petals
            .unwrap_or(5 + (self.rng.next() * 4.0) as u32)
            .clamp(3, 12);
    }

    /// Marchita la planta; al llegar al suelo renace con `seed`.
    pub fn wither(&mut self) {
        self.withering = true;
    }

    /// Punto del tallo para `s` en 0..1. Enredadera: ondula y se mece con los graves.
    pub fn stem_point(&self, s: f32) -> (f32, f32) {
        let amp = 0.075 * smoothstep(0.0, 0.3, s);
        let wave = (TAU * 1.1 * s + self.phase[0]).sin() * 0.7
            + (TAU * 2.7 * s + self.phase[1]).sin() * 0.3;
        let sway = 0.024 * s * s * (self.sway_clock + s * 2.5 + self.phase[2]).sin()
            * (0.35 + 0.9 * self.bands.bass + 1.6 * self.wind)
            + 0.006 * s * self.kick_pulse * (self.time * 9.0).sin();
        (amp * wave + sway, s * STEM_HEIGHT)
    }

    /// Geometría actual de la flor. Viaja en la punta del tallo desde el inicio.
    pub fn flower(&self) -> Flower {
        let (tx, ty) = self.stem_point(self.growth);
        let size = 0.20 + 0.80 * smoothstep(0.0, 1.0, self.growth);
        let pulse = 1.0
            + 0.16 * self.bands.treble
            + 0.10 * self.bands.level
            + 0.07 * self.hat_pulse
            + 0.04 * self.kick_pulse
            + 0.07 * self.bands.intensity
            + 0.14 * self.climax_pulse
            + 0.05 * self.beat_pulse;
        let open = (self.open + 0.10 * self.bands.mid * self.open.sqrt()).clamp(0.0, 1.0);
        let gene_size = self.genes.map_or(1.0, |g| g.size);
        let radius = FLOWER_RADIUS * size * (0.62 + 0.38 * open) * pulse * gene_size;
        Flower {
            x: tx,
            y: ty + radius * 0.10,
            radius,
            open,
            rot: self.rot,
            petals: self.genes.map_or(self.petals, |g| g.petals),
            genes: self.genes,
        }
    }

    /// Avanza la simulación.
    ///
    /// - `progress`: avance de la canción en 0..1, si se conoce.
    /// - `song_len`: duración en segundos (o una estimación).
    /// - `playing`: `false` en pausa: la planta deja de crecer.
    /// - `half_width`: medio ancho visible, en unidades de mundo (para las motas).
    pub fn update(
        &mut self,
        dt: f32,
        bands: &Bands,
        progress: Option<f32>,
        song_len: f32,
        playing: bool,
        half_width: f32,
    ) {
        self.time += dt;
        self.bands = *bands;
        self.kick_pulse = (self.kick_pulse * (-dt / 0.18).exp()).max(if bands.kick { 1.0 } else { 0.0 });
        self.hat_pulse = (self.hat_pulse * (-dt / 0.10).exp()).max(if bands.hat { 1.0 } else { 0.0 });
        self.climax_pulse = (self.climax_pulse * (-dt / 2.2).exp()).max(if bands.climax { 1.0 } else { 0.0 });
        self.beat_pulse = (self.beat_pulse * (-dt / 0.12).exp()).max(if bands.beat { 1.0 } else { 0.0 });

        // Vaivén al compás: un ir y venir cada dos pulsos. En cada pulso se corrige la fase
        // para que el tallo esté pasando por el centro justo en el golpe.
        let synced = bands.song.tempo_confidence > 0.3 && bands.song.bpm > 0.0;
        let rate = if synced { std::f32::consts::PI * bands.song.bpm / 60.0 } else { 1.1 };
        self.sway_clock += dt * rate;
        if synced && bands.beat {
            let half = std::f32::consts::PI;
            let off = (self.sway_clock + self.phase[2]).rem_euclid(half);
            self.sway_clock -= 0.5 * if off > half / 2.0 { off - half } else { off };
        }

        if self.withering {
            self.growth -= dt * 0.95;
            self.open = (self.open - dt * 1.4).max(0.0);
            if self.growth <= 0.0 {
                // Mezcla el estado propio: así cada planta del jardín renace distinta.
                let seed = (self.time * 1000.0) as u64 ^ self.rng.0;
                self.reseed(seed);
            }
        } else {
            self.grow(dt, bands, progress, song_len, playing);
            self.sprout_leaves(bands);
        }

        if bands.climax && !self.withering {
            self.burst();
        }

        // La flor gira lento; los medios la aceleran.
        self.rot += dt * (0.08 + 0.55 * bands.mid);

        // Pulsos de luz: nacen en la tierra con cada bombo y suben por el tallo.
        if bands.kick && !self.withering && self.growth > 0.03 {
            self.pulses.push(0.0);
        }
        for p in &mut self.pulses {
            *p += dt * 1.5;
        }
        let limit = self.growth + 0.12;
        self.pulses.retain(|p| *p < limit);

        for leaf in &mut self.leaves {
            leaf.age += dt;
        }
        for t in &mut self.tendrils {
            t.age += dt;
        }
        // Al marchitarse, lo que queda por encima de la punta desaparece.
        let g = self.growth;
        self.leaves.retain(|l| l.s <= g + 0.001);
        self.tendrils.retain(|t| t.s <= g + 0.001);

        self.emit_particles(dt, bands, half_width);
        self.move_particles(dt);
    }

    fn grow(&mut self, dt: f32, bands: &Bands, progress: Option<f32>, song_len: f32, playing: bool) {
        // Velocidad base: el tallo llega arriba pasada la mitad de la canción. Los graves la
        // modulan.
        let base = 1.0 / (GROW_END * song_len.max(20.0));
        let mut rate = if playing { base * (0.35 + 1.5 * bands.bass) } else { 0.0 };
        if let Some(p) = progress {
            // Corrección suave hacia el avance real, para que toda canción complete el arco.
            // Rápido al principio y más lento al acercarse arriba.
            let target = 1.0 - (1.0 - (p / GROW_END).clamp(0.0, 1.0)).powf(GROW_EASE);
            let gap = target - self.growth;
            rate += 0.10 * gap + 0.9 * (gap - 0.12).max(0.0);
        }
        self.growth = (self.growth + rate.max(0.0) * dt).clamp(0.0, 1.0);

        // Apertura: medios y agudos abren la flor; el tope sube con el tallo.
        let cap = 0.10 + 0.90 * smoothstep(0.0, 1.0, self.growth);
        let drive = 0.12 + 1.2 * bands.mid + 0.6 * bands.treble;
        self.open = (self.open + (cap - self.open).max(0.0) * drive * 0.5 * dt).clamp(0.0, 1.0);
    }

    fn sprout_leaves(&mut self, bands: &Bands) {
        // Dos cotiledones al brotar.
        if self.leaves.is_empty() && self.growth > 0.025 {
            for side in [-1.0, 1.0] {
                self.leaves.push(Leaf {
                    s: 0.02,
                    side,
                    size: 0.55,
                    angle: 0.5,
                    age: 0.0,
                    tint: self.rng.next(),
                });
            }
            self.last_leaf_s = 0.02;
        }
        // Una hoja por bombo, con separación mínima. Sin bombos, igual brota cada tanto.
        let gap = self.growth - self.last_leaf_s;
        let due = (bands.kick && gap > 0.10) || gap > 0.17;
        if due && self.growth > 0.07 && self.growth < 0.93 && self.leaves.len() < MAX_LEAVES {
            let s = self.growth - 0.015;
            self.leaf_side = -self.leaf_side;
            self.leaves.push(Leaf {
                s,
                side: self.leaf_side,
                size: self.rng.range(0.8, 1.25) * (1.0 - 0.35 * s),
                angle: self.rng.range(0.55, 1.0),
                age: 0.0,
                tint: self.rng.next(),
            });
            self.last_leaf_s = self.growth;
            if self.leaves.len() % 2 == 1 {
                self.tendrils.push(Tendril {
                    s: (s - 0.05).max(0.03),
                    side: -self.leaf_side,
                    size: self.rng.range(0.8, 1.3),
                    age: 0.0,
                });
            }
        }
    }

    fn emit_particles(&mut self, dt: f32, bands: &Bands, half_width: f32) {
        let f = self.flower();
        let visible = smoothstep(0.0, 0.25, self.growth) * if self.withering { 0.0 } else { 1.0 };

        // Polen continuo, proporcional a los agudos.
        self.pollen_debt +=
            dt * visible * (1.5 + 26.0 * bands.treble) * (0.3 + 0.7 * f.open) * (0.6 + 1.2 * bands.intensity);
        while self.pollen_debt >= 1.0 {
            self.pollen_debt -= 1.0;
            let a = self.rng.range(0.0, TAU);
            let r = f.radius * self.rng.range(0.1, 0.6);
            let v = self.rng.range(0.03, 0.10);
            let p = Particle {
                x: f.x + a.cos() * r,
                y: f.y + a.sin() * r,
                vx: a.cos() * v,
                vy: a.sin() * v + 0.03,
                age: 0.0,
                life: self.rng.range(0.9, 2.4),
                kind: ParticleKind::Pollen,
                phase: self.rng.range(0.0, TAU),
                variant: 0,
            };
            self.spawn(p);
        }

        // Estallido de chispas en cada platillo.
        if bands.hat && visible > 0.3 {
            let n = 3 + (7.0 * bands.treble * f.open) as usize;
            for _ in 0..n {
                let a = self.rng.range(0.0, TAU);
                let v = self.rng.range(0.16, 0.38);
                let p = Particle {
                    x: f.x + a.cos() * f.radius * 0.8,
                    y: f.y + a.sin() * f.radius * 0.8,
                    vx: a.cos() * v,
                    vy: a.sin() * v,
                    age: 0.0,
                    life: self.rng.range(0.35, 0.8),
                    kind: ParticleKind::Spark,
                    phase: 0.0,
                    variant: (self.rng.next() * 3.0) as u8,
                };
                self.spawn(p);
            }
        }

        // Algunos bombos sueltan un pétalo.
        if bands.kick && f.open > 0.55 && visible > 0.5 && self.rng.next() < 0.4 {
            let a = self.rng.range(PI, TAU); // mitad inferior
            let p = Particle {
                x: f.x + a.cos() * f.radius * 0.9,
                y: f.y + a.sin() * f.radius * 0.9,
                vx: self.rng.range(-0.05, 0.05),
                vy: -0.02,
                age: 0.0,
                life: self.rng.range(3.5, 6.0),
                kind: ParticleKind::Petal,
                phase: self.rng.range(0.0, TAU),
                variant: (self.rng.next() * 3.0) as u8,
            };
            self.spawn(p);
        }

        // Motas de fondo: siempre unas pocas flotando.
        let motes = self.particles.iter().filter(|p| p.kind == ParticleKind::Mote).count();
        if self.ambient && motes < 14 && self.rng.next() < dt * 3.0 {
            let p = Particle {
                x: self.rng.range(-half_width, half_width),
                y: self.rng.range(0.05, 0.98),
                vx: self.rng.range(-0.008, 0.008),
                vy: self.rng.range(0.004, 0.02),
                age: 0.0,
                life: self.rng.range(4.0, 9.0),
                kind: ParticleKind::Mote,
                phase: self.rng.range(0.0, TAU),
                variant: 0,
            };
            self.spawn(p);
        }
    }

    /// Clímax: la flor se abre de golpe, sueltan pétalos y chispas, y el tallo se enciende.
    fn burst(&mut self) {
        self.open = (self.open + 0.25).min(1.0);
        let f = self.flower();
        let visible = smoothstep(0.0, 0.2, self.growth);
        for i in 0..(46.0 * visible) as usize {
            let a = self.rng.range(0.0, TAU);
            let petal = i % 6 == 0;
            let v = if petal { self.rng.range(0.05, 0.16) } else { self.rng.range(0.2, 0.55) };
            let p = Particle {
                x: f.x + a.cos() * f.radius * 0.7,
                y: f.y + a.sin() * f.radius * 0.7,
                vx: a.cos() * v,
                vy: a.sin() * v + if petal { 0.06 } else { 0.0 },
                age: 0.0,
                life: if petal { self.rng.range(3.0, 5.5) } else { self.rng.range(0.6, 1.4) },
                kind: if petal { ParticleKind::Petal } else { ParticleKind::Spark },
                phase: self.rng.range(0.0, TAU),
                variant: (self.rng.next() * 3.0) as u8,
            };
            self.spawn(p);
        }
        // Tres pulsos de luz seguidos suben por el tallo.
        self.pulses.extend([0.0, -0.12, -0.24]);
    }

    fn spawn(&mut self, p: Particle) {
        if self.particles.len() < MAX_PARTICLES {
            self.particles.push(p);
        }
    }

    fn move_particles(&mut self, dt: f32) {
        let time = self.time;
        for p in &mut self.particles {
            p.age += dt;
            match p.kind {
                ParticleKind::Pollen => {
                    let drag = (-dt * 1.2).exp();
                    p.vx *= drag;
                    p.vy = p.vy * drag + 0.02 * dt;
                    p.x += (p.phase + time * 2.0).sin() * 0.012 * dt;
                }
                ParticleKind::Spark => {
                    let drag = (-dt * 2.6).exp();
                    p.vx *= drag;
                    p.vy *= drag;
                }
                ParticleKind::Petal => {
                    p.vy = (p.vy - 0.05 * dt).max(-0.09);
                    p.vx = p.vx * (-dt * 0.8).exp() + (p.phase + p.age * 2.2).sin() * 0.05 * dt;
                }
                ParticleKind::Mote => {}
            }
            p.x += p.vx * dt;
            p.y += p.vy * dt;
        }
        self.particles.retain(|p| p.age < p.life && p.y > 0.0 && p.y < 1.2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steady(bass: f32) -> Bands {
        Bands { bass, mid: 0.4, treble: 0.3, level: 0.5, ..Bands::default() }
    }

    /// Simula una canción completa y devuelve la planta.
    fn play(song_len: f32, bass: f32, upto: f32) -> Plant {
        let mut plant = Plant::new(7, None);
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        while t < song_len * upto {
            let mut b = steady(bass);
            b.kick = ((t * 2.0) as u32) != (((t - dt) * 2.0) as u32); // 120 BPM
            plant.update(dt, &b, Some(t / song_len), song_len, true, 1.0);
            t += dt;
        }
        plant
    }

    #[test]
    fn stem_reaches_top_a_bit_past_half_the_song() {
        for bass in [0.05, 0.45, 0.9] {
            let p = play(180.0, bass, 0.66);
            assert!(p.growth > 0.93, "bass={bass}: growth={}", p.growth);
        }
        // Un poco más rápido al principio, sin terminar de inmediato: al 25% de la canción
        // va cerca de la mitad, aun con mucho bajo.
        let quarter = play(180.0, 0.45, 0.25).growth;
        assert!((0.4..0.65).contains(&quarter), "growth={quarter}");
        assert!(play(180.0, 0.9, 0.25).growth < 0.75);
    }

    #[test]
    fn bass_speeds_up_growth() {
        let quiet = play(180.0, 0.05, 0.15).growth;
        let loud = play(180.0, 0.9, 0.15).growth;
        assert!(loud > quiet + 0.03, "quiet={quiet} loud={loud}");
    }

    #[test]
    fn flower_is_a_bud_at_start_and_open_at_end() {
        let early = play(180.0, 0.4, 0.02).flower();
        let late = play(180.0, 0.4, 0.95).flower();
        assert!(early.radius < late.radius * 0.5);
        assert!(early.open < 0.3 && late.open > 0.85, "{} {}", early.open, late.open);
    }

    #[test]
    fn leaves_sprout_and_wither_resets() {
        let mut p = play(120.0, 0.5, 0.8);
        assert!(p.leaves.len() >= 6, "hojas: {}", p.leaves.len());
        p.wither();
        for _ in 0..200 {
            p.update(1.0 / 60.0, &steady(0.3), Some(0.0), 120.0, true, 1.0);
        }
        assert!(!p.withering && p.growth < 0.1 && p.leaves.len() <= 2);
    }

    #[test]
    fn genes_come_from_the_song() {
        let song = |bpm: f32, majorness: f32, arousal: f32, key: u8| Song { bpm, majorness, arousal, key, ..Song::default() };
        let slow_minor = Genes::from_song(&song(70.0, -0.8, 0.2, 9));
        let fast_major = Genes::from_song(&song(150.0, 0.8, 0.8, 0));
        assert_eq!((slow_minor.petals, slow_minor.layers), (5, 2));
        assert_eq!((fast_major.petals, fast_major.layers), (8, 3));
        assert!(slow_minor.sharp > 0.5 && fast_major.sharp < -0.5);
        assert!(fast_major.size > slow_minor.size);
        // Misma canción, misma flor.
        assert_eq!(Genes::from_song(&song(70.0, -0.8, 0.2, 9)), slow_minor);
        // La flor usa los genes en cuanto los tiene.
        let mut p = Plant::new(1, None);
        p.genes = Some(fast_major);
        assert_eq!(p.flower().petals, 8);
    }

    #[test]
    fn paused_plant_settles_and_stops() {
        // En pausa la planta termina de alcanzar la altura que le toca y ahí se queda.
        let mut p = play(180.0, 0.5, 0.15);
        let due = 1.0 - (1.0 - 0.15 / GROW_END).powf(GROW_EASE);
        let pause = |p: &mut Plant, frames: usize| {
            for _ in 0..frames {
                p.update(1.0 / 60.0, &Bands::default(), Some(0.15), 180.0, false, 1.0);
            }
        };
        pause(&mut p, 1800);
        let settled = p.growth;
        assert!(settled <= due + 0.005, "pasó de largo: {settled} > {due}");
        pause(&mut p, 1800);
        assert!((p.growth - settled).abs() < 0.01, "sigue creciendo en pausa");
    }
}

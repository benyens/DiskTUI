//! Jardín: la planta principal y sus compañeras más pequeñas a los lados.

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::palette::Palette;
use crate::plant::{Genes, Plant};
use crate::render::{self, View};
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// A qué parte de la música responde más una planta.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Voice {
    /// Planta principal: responde a todo.
    Full,
    /// Su flor pulsa con los medios (voz, guitarras).
    Mids,
    /// Su flor pulsa con los agudos (platillos).
    Highs,
}

pub struct Member {
    pub plant: Plant,
    /// Posición horizontal, en unidades de mundo de la planta principal.
    pub x: f32,
    /// Tamaño relativo a la principal.
    pub scale: f32,
    /// Fracción de la canción en la que brota.
    pub delay: f32,
    pub voice: Voice,
}

impl Member {
    /// Bandas tal como las "oye" esta planta.
    fn hear(&self, b: &Bands) -> Bands {
        match self.voice {
            Voice::Full => *b,
            // La de la izquierda oye más el canal izquierdo; la de la derecha, el derecho.
            Voice::Mids => {
                let side = (1.0 - 0.8 * b.pan).clamp(0.4, 1.6);
                Bands { treble: (b.mid * side).min(1.0), mid: (b.mid * side).min(1.0), level: (b.level * side).min(1.0), hat: false, ..*b }
            }
            Voice::Highs => {
                let side = (1.0 + 0.8 * b.pan).clamp(0.4, 1.6);
                Bands {
                    bass: b.bass * 0.7,
                    mid: (b.treble * side).min(1.0),
                    treble: (b.treble * side).min(1.0),
                    level: (b.level * side).min(1.0),
                    kick: false,
                    ..*b
                }
            }
        }
    }

    /// Colores de esta planta: las laterales intercambian roles y se apagan un poco,
    /// para dar profundidad y usar más colores de la carátula.
    pub fn palette(&self, pal: &Palette) -> Palette {
        let mut p = *pal;
        // La tónica de la canción corre un poco el tono de los pétalos.
        if let Some(g) = self.plant.genes {
            let (h, s, l) = p.petal.to_hsl();
            p.petal = crate::color::Rgb::from_hsl(h + g.hue, s, l);
            p.petal_light = p.petal.mix(crate::color::Rgb::WHITE, 0.4);
        }
        match self.voice {
            Voice::Full => return p,
            Voice::Mids => {
                p.petal = pal.accent;
                p.accent = pal.petal_light;
            }
            Voice::Highs => {
                p.petal = pal.petal.mix(pal.accent, 0.5);
                p.accent = pal.petal_light.mix(pal.accent, 0.3);
            }
        }
        p.petal_light = p.petal.mix(crate::color::Rgb::WHITE, 0.4);
        let far = pal.bg_top;
        p.petal = p.petal.mix(far, 0.14);
        p.petal_light = p.petal_light.mix(far, 0.14);
        p.accent = p.accent.mix(far, 0.10);
        p.stem = p.stem.mix(far, 0.22);
        p.leaf = p.leaf.mix(far, 0.22);
        p
    }
}

pub struct Garden {
    /// La principal va primero.
    pub members: Vec<Member>,
    /// Cielo, cerros, flores silvestres, luciérnagas y mariposas.
    pub scenery: Scenery,
    /// Los pétalos los fijó `--petalos`: la canción no los cambia.
    fixed_petals: bool,
}

impl Garden {
    /// `flowers`: 1 a 3. `petals` fija los pétalos de la flor principal.
    pub fn new(seed: u64, flowers: u32, petals: Option<u32>) -> Self {
        let mut members = vec![Member {
            plant: Plant::new(seed, petals),
            x: 0.0,
            scale: 1.0,
            delay: 0.0,
            voice: Voice::Full,
        }];
        let sides = [
            (-0.47, 0.62, 0.05, Voice::Mids),
            (0.45, 0.50, 0.10, Voice::Highs),
        ];
        for (i, (x, scale, delay, voice)) in sides.into_iter().take(flowers.saturating_sub(1) as usize).enumerate() {
            let mut plant = Plant::new(seed.rotate_left(17) ^ (0xA5A5 + i as u64 * 7919), None);
            plant.ambient = false;
            members.push(Member { plant, x, scale, delay, voice });
        }
        Garden { members, scenery: Scenery::new(seed), fixed_petals: petals.is_some() }
    }

    pub fn main(&self) -> &Plant {
        &self.members[0].plant
    }

    /// Medio ancho que ocupa el jardín, en unidades de mundo de la planta principal.
    pub fn half_span(&self) -> f32 {
        if self.members.len() > 1 { 0.66 } else { 0.42 }
    }

    pub fn wither(&mut self) {
        for m in &mut self.members {
            m.plant.wither();
        }
        self.scenery.fade();
    }

    /// Avanza todas las plantas. Las laterales brotan más tarde y cuentan su propio avance.
    pub fn update(
        &mut self,
        dt: f32,
        bands: &Bands,
        progress: Option<f32>,
        song_len: f32,
        playing: bool,
        half_width: f32,
    ) {
        // Flor única: cuando el análisis de la canción ya es fiable, sus rasgos quedan
        // fijos hasta la próxima canción. En modo archivo pasa en el primer segundo; en
        // Spotify, a los ~20 s de escucha.
        if bands.song.settled >= 0.95 && self.members[0].plant.genes.is_none() && !self.members[0].plant.withering {
            let mut genes = Genes::from_song(&bands.song);
            if self.fixed_petals {
                genes.petals = self.members[0].plant.petals;
            }
            for (i, m) in self.members.iter_mut().enumerate() {
                m.plant.genes = Some(match i {
                    0 => genes,
                    1 => genes.sibling(-1),
                    _ => genes.sibling(2),
                });
            }
        }

        // El clima marca el paso: las canciones tristes crecen más lento, las alegres más rápido.
        let pace = self.scenery.atmosphere().growth;
        let wind = self.scenery.wind();
        for m in &mut self.members {
            let heard = m.hear(bands);
            let own = progress.map(|p| ((p * pace - m.delay) / (1.0 - m.delay)).clamp(0.0, 1.0));
            let started = progress.is_none_or(|p| p * pace >= m.delay);
            m.plant.wind = wind;
            m.plant.update(dt, &heard, own, song_len / pace, playing && started, half_width);
        }
        // Centros de las flores en coordenadas de la planta principal: destino de las mariposas.
        let flowers: Vec<(f32, f32)> = self
            .members
            .iter()
            .filter(|m| m.plant.growth > 0.3)
            .map(|m| {
                let f = m.plant.flower();
                (m.x + f.x * m.scale, f.y * m.scale)
            })
            .collect();
        self.scenery.update(dt, bands, progress, playing, half_width, &flowers);
    }
}

impl Scene for Garden {
    fn update(&mut self, f: &Frame, w: usize, h: usize) {
        let half_width = View::for_size(w, h, self.half_span()).half_width();
        Garden::update(self, f.dt, f.bands, f.progress, f.song_len, f.playing, half_width);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        render::draw(canvas, self, pal);
    }

    fn new_track(&mut self) {
        self.wither();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(garden: &mut Garden, from: f32, to: f32, len: f32) {
        let dt = 1.0 / 60.0;
        let mut t = from * len;
        while t < to * len {
            let b = Bands { bass: 0.5, mid: 0.5, treble: 0.4, level: 0.5, ..Bands::default() };
            garden.update(dt, &b, Some(t / len), len, true, 1.0);
            t += dt;
        }
    }

    #[test]
    fn side_plants_sprout_later_and_all_finish() {
        let mut g = Garden::new(1, 3, None);
        run(&mut g, 0.0, 0.04, 120.0);
        assert!(g.members[0].plant.growth > 0.03);
        assert!(g.members[1].plant.growth < 0.005 && g.members[2].plant.growth < 0.005);
        run(&mut g, 0.04, 0.75, 120.0);
        for m in &g.members {
            assert!(m.plant.growth > 0.93, "growth {}", m.plant.growth);
        }
    }

    #[test]
    fn side_plants_differ_from_each_other() {
        let g = Garden::new(42, 3, None);
        let a = g.members[1].plant.stem_point(0.7);
        let b = g.members[2].plant.stem_point(0.7);
        assert!((a.0 - b.0).abs() > 1e-4, "misma forma: {a:?} {b:?}");
    }

    #[test]
    fn one_flower_means_one_plant() {
        assert_eq!(Garden::new(1, 1, None).members.len(), 1);
        assert_eq!(Garden::new(1, 2, None).members.len(), 2);
    }
}

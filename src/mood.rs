//! Del ánimo de la canción al clima de la escena.
//!
//! El ánimo vive en un plano: valencia (triste ↔ alegre) y activación (calmada ↔ enérgica).
//! Cinco climas marcan puntos de ese plano y la escena se mezcla entre ellos de forma
//! continua, sin saltos: una canción a medio camino entre triste y nostálgica da una noche
//! con llovizna.

use crate::color::Rgb;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    /// Alegre y enérgica: día soleado.
    Happy,
    /// Alegre y calmada: atardecer dorado.
    Serene,
    /// Intermedia: noche despejada con luna.
    Nostalgic,
    /// Triste y calmada: lluvia.
    Sad,
    /// Oscura y enérgica: tormenta.
    Intense,
}

impl Mood {
    pub const ALL: [Mood; 5] = [Mood::Happy, Mood::Serene, Mood::Nostalgic, Mood::Sad, Mood::Intense];

    /// Centro de cada ánimo en el plano (valencia, activación).
    pub fn center(self) -> (f32, f32) {
        match self {
            Mood::Happy => (0.76, 0.74),
            Mood::Serene => (0.72, 0.28),
            Mood::Nostalgic => (0.48, 0.44),
            Mood::Sad => (0.24, 0.24),
            Mood::Intense => (0.24, 0.78),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Mood::Happy => "alegre",
            Mood::Serene => "serena",
            Mood::Nostalgic => "nostálgica",
            Mood::Sad => "triste",
            Mood::Intense => "intensa",
        }
    }

    pub fn parse(name: &str) -> Option<Mood> {
        match name.to_lowercase().as_str() {
            "alegre" | "feliz" | "soleado" => Some(Mood::Happy),
            "serena" | "tranquila" | "atardecer" => Some(Mood::Serene),
            "nostalgica" | "nostálgica" | "noche" => Some(Mood::Nostalgic),
            "triste" | "lluvia" => Some(Mood::Sad),
            "intensa" | "tormenta" => Some(Mood::Intense),
            _ => None,
        }
    }

    /// El ánimo más cercano a un punto del plano.
    pub fn nearest(valence: f32, arousal: f32) -> Mood {
        let dist = |m: Mood| {
            let (v, a) = m.center();
            (v - valence).powi(2) + (a - arousal).powi(2)
        };
        Mood::ALL.into_iter().min_by(|a, b| dist(*a).total_cmp(&dist(*b))).unwrap_or(Mood::Nostalgic)
    }

    /// El clima puro de este ánimo.
    fn atmosphere(self) -> Atmosphere {
        match self {
            Mood::Happy => Atmosphere {
                sky_top: Rgb(0.10, 0.30, 0.62),
                sky_bottom: Rgb(0.42, 0.66, 0.86),
                daylight: 1.0,
                sun: 1.0,
                sun_height: 0.80,
                moon: 0.0,
                stars: 0.0,
                aurora: 0.0,
                clouds: 0.30,
                cloud_tone: Rgb(0.95, 0.96, 1.0),
                rain: 0.0,
                wind: 0.25,
                storm: 0.0,
                fireflies: 0.0,
                butterflies: 1.0,
                growth: 1.25,
            },
            Mood::Serene => Atmosphere {
                sky_top: Rgb(0.13, 0.12, 0.30),
                sky_bottom: Rgb(0.86, 0.46, 0.24),
                daylight: 0.55,
                sun: 1.0,
                sun_height: 0.26,
                moon: 0.0,
                stars: 0.15,
                aurora: 0.0,
                clouds: 0.35,
                cloud_tone: Rgb(0.96, 0.62, 0.50),
                rain: 0.0,
                wind: 0.10,
                storm: 0.0,
                fireflies: 0.45,
                butterflies: 0.6,
                growth: 0.90,
            },
            Mood::Nostalgic => Atmosphere {
                sky_top: Rgb(0.020, 0.030, 0.060),
                sky_bottom: Rgb(0.040, 0.060, 0.075),
                daylight: 0.0,
                sun: 0.0,
                sun_height: 0.0,
                moon: 1.0,
                stars: 1.0,
                aurora: 0.7,
                clouds: 0.0,
                cloud_tone: Rgb(0.20, 0.22, 0.30),
                rain: 0.0,
                wind: 0.10,
                storm: 0.0,
                fireflies: 1.0,
                butterflies: 0.5,
                growth: 0.90,
            },
            Mood::Sad => Atmosphere {
                sky_top: Rgb(0.045, 0.060, 0.085),
                sky_bottom: Rgb(0.130, 0.160, 0.200),
                daylight: 0.12,
                sun: 0.0,
                sun_height: 0.0,
                moon: 0.2,
                stars: 0.0,
                aurora: 0.0,
                clouds: 0.90,
                cloud_tone: Rgb(0.22, 0.25, 0.31),
                rain: 0.75,
                wind: 0.20,
                storm: 0.0,
                fireflies: 0.1,
                butterflies: 0.0,
                growth: 0.65,
            },
            Mood::Intense => Atmosphere {
                sky_top: Rgb(0.035, 0.030, 0.075),
                sky_bottom: Rgb(0.160, 0.095, 0.200),
                daylight: 0.05,
                sun: 0.0,
                sun_height: 0.0,
                moon: 0.0,
                stars: 0.0,
                aurora: 0.25,
                clouds: 1.0,
                cloud_tone: Rgb(0.17, 0.14, 0.24),
                rain: 0.55,
                wind: 1.0,
                storm: 1.0,
                fireflies: 0.0,
                butterflies: 0.0,
                growth: 1.20,
            },
        }
    }
}

/// Parámetros continuos del clima. Cada campo va de 0 a 1 salvo los colores y `growth`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Atmosphere {
    pub sky_top: Rgb,
    pub sky_bottom: Rgb,
    /// Cuánta luz de día hay: apaga resplandores y aclara los cerros.
    pub daylight: f32,
    pub sun: f32,
    /// Altura del sol: 0 en el horizonte, 1 arriba.
    pub sun_height: f32,
    pub moon: f32,
    pub stars: f32,
    pub aurora: f32,
    pub clouds: f32,
    pub cloud_tone: Rgb,
    pub rain: f32,
    pub wind: f32,
    /// Tormenta: habilita los relámpagos en clímax y golpes fuertes.
    pub storm: f32,
    pub fireflies: f32,
    pub butterflies: f32,
    /// Factor de velocidad de crecimiento: las canciones tristes crecen más lento.
    pub growth: f32,
}

impl Default for Atmosphere {
    /// La noche con luna: el aspecto por defecto mientras no se conoce el ánimo.
    fn default() -> Self {
        Mood::Nostalgic.atmosphere()
    }
}

/// Cuánto mueve la energía de la sección el ánimo hacia arriba o abajo en el eje de
/// activación: con energía 1 sube `SECTION_SWING / 2`, con 0 baja lo mismo.
pub const SECTION_SWING: f32 = 0.8;

/// Ánimo del momento: el de la canción, movido según la energía de la sección que suena.
/// Un verso tranquilo calma el clima; un coro fuerte lo agita.
pub fn in_section(valence: f32, arousal: f32, energy: f32) -> (f32, f32) {
    let shift = (energy.clamp(0.0, 1.0) - 0.5) * SECTION_SWING;
    (valence, (arousal + shift).clamp(0.0, 1.0))
}

/// Clima de una sección: el de la canción (`base`), llevado hacia su versión más agitada en
/// las partes fuertes y hacia la más calma en las suaves. Las canciones de valencia baja se
/// agitan hacia la tormenta y se calman hacia la lluvia; las de valencia alta, hacia el sol
/// pleno y el atardecer. Devuelve también el ánimo del momento.
pub fn section_weather(base: &Atmosphere, song: Mood, valence: f32, energy: f32, trust: f32) -> (Atmosphere, Mood) {
    let bright = ((valence - 0.30) / 0.40).clamp(0.0, 1.0);
    let lift = (energy.clamp(0.0, 1.0) - 0.5) * 2.0;
    let (dark, light) = if lift >= 0.0 { (Mood::Intense, Mood::Happy) } else { (Mood::Sad, Mood::Serene) };
    let mut toward = Atmosphere::of(dark);
    toward.approach(&Atmosphere::of(light), bright);
    let weight = trust * (lift.abs() * 0.9).min(0.85);
    let mut out = *base;
    out.approach(&toward, weight);
    let moment = if weight > 0.4 { if bright < 0.5 { dark } else { light } } else { song };
    (out, moment)
}

/// Energía de la sección: la intensidad suavizada, para que siga a las partes de la canción
/// (verso, coro) y no a cada golpe.
#[derive(Clone, Copy, Debug)]
pub struct SectionEnergy(f32);

impl Default for SectionEnergy {
    fn default() -> Self {
        SectionEnergy(0.5)
    }
}

impl SectionEnergy {
    /// Tiempo de respuesta, en segundos: una sección nueva se nota en unos 5 s.
    const TAU: f32 = 4.0;

    pub fn update(&mut self, intensity: f32, climax: bool, playing: bool, dt: f32) {
        if !playing {
            return;
        }
        // Un clímax es la sección más intensa: entra de golpe en vez de esperar.
        let target = if climax { 1.0 } else { intensity.clamp(0.0, 1.0) };
        let k = if climax { 0.5 } else { 1.0 - (-dt / Self::TAU).exp() };
        self.0 += (target - self.0) * k;
    }

    pub fn get(self) -> f32 {
        self.0
    }

    /// Canción nueva: vuelve al medio.
    pub fn reset(&mut self) {
        self.0 = 0.5;
    }
}

impl Atmosphere {
    /// Clima para un punto del plano: mezcla de los cinco, cada uno pesado por cercanía.
    pub fn for_mood(valence: f32, arousal: f32) -> Atmosphere {
        let weights: Vec<(Atmosphere, f32)> = Mood::ALL
            .into_iter()
            .map(|m| {
                let (v, a) = m.center();
                let d2 = (v - valence).powi(2) + (a - arousal).powi(2);
                (m.atmosphere(), (-d2 / (2.0 * 0.15f32.powi(2))).exp())
            })
            .collect();
        let total: f32 = weights.iter().map(|w| w.1).sum::<f32>().max(1e-9);
        let mix = |f: fn(&Atmosphere) -> f32| weights.iter().map(|(a, w)| f(a) * w).sum::<f32>() / total;
        let mix_color = |f: fn(&Atmosphere) -> Rgb| {
            let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
            for (a, w) in &weights {
                let c = f(a);
                r += c.0 * w;
                g += c.1 * w;
                b += c.2 * w;
            }
            Rgb(r / total, g / total, b / total)
        };
        Atmosphere {
            sky_top: mix_color(|a| a.sky_top),
            sky_bottom: mix_color(|a| a.sky_bottom),
            daylight: mix(|a| a.daylight),
            sun: mix(|a| a.sun),
            sun_height: mix(|a| a.sun_height * a.sun) / mix(|a| a.sun).max(1e-3),
            moon: mix(|a| a.moon),
            stars: mix(|a| a.stars),
            aurora: mix(|a| a.aurora),
            clouds: mix(|a| a.clouds),
            cloud_tone: mix_color(|a| a.cloud_tone),
            rain: mix(|a| a.rain),
            wind: mix(|a| a.wind),
            storm: mix(|a| a.storm),
            fireflies: mix(|a| a.fireflies),
            butterflies: mix(|a| a.butterflies),
            growth: mix(|a| a.growth),
        }
    }

    pub fn of(mood: Mood) -> Atmosphere {
        mood.atmosphere()
    }

    /// Avanza hacia `target` una fracción `t`: el clima cambia de a poco.
    pub fn approach(&mut self, target: &Atmosphere, t: f32) {
        let t = t.clamp(0.0, 1.0);
        let l = |a: f32, b: f32| a + (b - a) * t;
        *self = Atmosphere {
            sky_top: self.sky_top.mix(target.sky_top, t),
            sky_bottom: self.sky_bottom.mix(target.sky_bottom, t),
            daylight: l(self.daylight, target.daylight),
            sun: l(self.sun, target.sun),
            sun_height: l(self.sun_height, target.sun_height),
            moon: l(self.moon, target.moon),
            stars: l(self.stars, target.stars),
            aurora: l(self.aurora, target.aurora),
            clouds: l(self.clouds, target.clouds),
            cloud_tone: self.cloud_tone.mix(target.cloud_tone, t),
            rain: l(self.rain, target.rain),
            wind: l(self.wind, target.wind),
            storm: l(self.storm, target.storm),
            fireflies: l(self.fireflies, target.fireflies),
            butterflies: l(self.butterflies, target.butterflies),
            growth: l(self.growth, target.growth),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_mood_center_gives_its_own_weather() {
        for mood in Mood::ALL {
            let (v, a) = mood.center();
            assert_eq!(Mood::nearest(v, a), mood);
            let got = Atmosphere::for_mood(v, a);
            let pure = Atmosphere::of(mood);
            assert!((got.rain - pure.rain).abs() < 0.12, "{}: lluvia {} vs {}", mood.name(), got.rain, pure.rain);
            assert!((got.daylight - pure.daylight).abs() < 0.12, "{}: luz {}", mood.name(), got.daylight);
        }
    }

    #[test]
    fn sad_rains_and_happy_shines() {
        let sad = Atmosphere::for_mood(0.2, 0.2);
        let happy = Atmosphere::for_mood(0.8, 0.8);
        assert!(sad.rain > 0.6 && sad.sun < 0.1 && sad.growth < 0.75);
        assert!(happy.sun > 0.9 && happy.rain < 0.05 && happy.daylight > 0.9 && happy.growth > 1.1);
        let storm = Atmosphere::for_mood(0.2, 0.85);
        assert!(storm.storm > 0.8 && storm.wind > 0.8);
    }

    #[test]
    fn weather_blends_between_moods() {
        // A medio camino entre triste y nostálgica: algo de lluvia, algo de luna.
        let (s, n) = (Mood::Sad.center(), Mood::Nostalgic.center());
        let mid = Atmosphere::for_mood((s.0 + n.0) / 2.0, (s.1 + n.1) / 2.0);
        assert!(mid.rain > 0.15 && mid.rain < 0.6, "lluvia {}", mid.rain);
        assert!(mid.moon > 0.3 && mid.moon < 0.9, "luna {}", mid.moon);
    }

    #[test]
    fn approach_converges() {
        let mut a = Atmosphere::default();
        let target = Atmosphere::of(Mood::Happy);
        for _ in 0..200 {
            a.approach(&target, 0.05);
        }
        assert!((a.sun - 1.0).abs() < 0.01 && a.sky_top.dist2(target.sky_top) < 1e-4);
    }

    #[test]
    fn names_roundtrip() {
        for mood in Mood::ALL {
            assert_eq!(Mood::parse(mood.name()), Some(mood));
        }
        assert_eq!(Mood::parse("jazz"), None);
    }

    #[test]
    fn loud_sections_stir_the_weather_and_quiet_ones_calm_it() {
        // Una canción triste: el coro fuerte la acerca a la tormenta, el verso suave a la calma.
        let (v, a) = Mood::Sad.center();
        let chorus = in_section(v, a, 1.0);
        let verse = in_section(v, a, 0.0);
        assert!(chorus.1 > a + 0.2 && verse.1 < a);
        assert_eq!(chorus.0, v, "la valencia es de la canción, no de la sección");
        let storm = |p: (f32, f32)| Atmosphere::for_mood(p.0, p.1).storm;
        assert!(storm(chorus) > storm(verse) + 0.1, "{} vs {}", storm(chorus), storm(verse));

        // El clima: el coro de una canción triste trae tormenta sin quitar la lluvia; el
        // verso de una alegre baja el sol hacia el atardecer.
        let sad = Atmosphere::of(Mood::Sad);
        let (loud, now) = section_weather(&sad, Mood::Sad, v, 1.0, 1.0);
        assert!(loud.storm > sad.storm + 0.3 && loud.rain >= sad.rain * 0.6, "{loud:?}");
        assert_eq!(now, Mood::Intense);
        let happy = Atmosphere::of(Mood::Happy);
        let (quiet, now) = section_weather(&happy, Mood::Happy, Mood::Happy.center().0, 0.0, 1.0);
        assert!(quiet.sun_height < happy.sun_height - 0.1, "{quiet:?}");
        assert_eq!(now, Mood::Serene);
        let (same, now) = section_weather(&happy, Mood::Happy, 0.76, 0.5, 1.0);
        assert!((same.sun_height - happy.sun_height).abs() < 1e-4 && now == Mood::Happy);

        let mut energy = SectionEnergy::default();
        for _ in 0..600 {
            energy.update(0.9, false, true, 1.0 / 60.0);
        }
        assert!(energy.get() > 0.85, "{}", energy.get());
        let before = energy.get();
        energy.update(0.0, false, false, 1.0);
        assert_eq!(energy.get(), before, "en pausa no cambia");
        energy.reset();
        energy.update(0.0, true, true, 1.0 / 60.0);
        assert!(energy.get() >= 0.75, "el clímax entra de golpe: {}", energy.get());
    }
}

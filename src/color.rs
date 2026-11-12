//! Color RGB en punto flotante (0..1) y conversión a colores de terminal.

use ratatui::style::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb(pub f32, pub f32, pub f32);

impl Rgb {
    pub const BLACK: Rgb = Rgb(0.0, 0.0, 0.0);
    pub const WHITE: Rgb = Rgb(1.0, 1.0, 1.0);

    pub fn from_u8(r: u8, g: u8, b: u8) -> Self {
        Rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }

    pub fn to_u8(self) -> (u8, u8, u8) {
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        (q(self.0), q(self.1), q(self.2))
    }

    /// Interpolación lineal: `t = 0` devuelve `self`, `t = 1` devuelve `o`.
    pub fn mix(self, o: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        Rgb(
            self.0 + (o.0 - self.0) * t,
            self.1 + (o.1 - self.1) * t,
            self.2 + (o.2 - self.2) * t,
        )
    }

    pub fn scale(self, k: f32) -> Rgb {
        Rgb(self.0 * k, self.1 * k, self.2 * k)
    }

    /// Suma aditiva (para brillos y resplandor). Se satura al convertir.
    pub fn add(self, o: Rgb, k: f32) -> Rgb {
        Rgb(self.0 + o.0 * k, self.1 + o.1 * k, self.2 + o.2 * k)
    }

    /// Distancia euclídea al cuadrado, para comparar colores.
    pub fn dist2(self, o: Rgb) -> f32 {
        let (a, b, c) = (self.0 - o.0, self.1 - o.1, self.2 - o.2);
        a * a + b * b + c * c
    }

    /// Devuelve (tono 0..1, saturación 0..1, luminosidad 0..1).
    pub fn to_hsl(self) -> (f32, f32, f32) {
        let (r, g, b) = (self.0, self.1, self.2);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        let d = max - min;
        if d < 1e-5 {
            return (0.0, 0.0, l);
        }
        let s = d / (1.0 - (2.0 * l - 1.0).abs()).max(1e-5);
        let h = if max == r {
            ((g - b) / d).rem_euclid(6.0)
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h / 6.0, s.clamp(0.0, 1.0), l)
    }

    pub fn from_hsl(h: f32, s: f32, l: f32) -> Rgb {
        let h = h.rem_euclid(1.0) * 6.0;
        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = c * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
        let (r, g, b) = match h as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = l - c / 2.0;
        Rgb(r + m, g + m, b + m)
    }
}

/// Capacidad de color de la terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    /// 24 bits (Ghostty, iTerm2, Kitty, WezTerm...).
    True,
    /// Paleta xterm de 256 colores, como respaldo.
    Ansi256,
}

impl ColorMode {
    /// Lee `COLORTERM`. Si no anuncia truecolor, usa 256 colores.
    pub fn detect() -> Self {
        match std::env::var("COLORTERM") {
            Ok(v) if v.contains("truecolor") || v.contains("24bit") => ColorMode::True,
            _ => ColorMode::Ansi256,
        }
    }

    pub fn color(self, c: Rgb) -> Color {
        let (r, g, b) = c.to_u8();
        match self {
            ColorMode::True => Color::Rgb(r, g, b),
            ColorMode::Ansi256 => Color::Indexed(rgb_to_ansi256(r, g, b)),
        }
    }
}

/// Color más cercano en la paleta xterm-256 (cubo 6×6×6 más rampa de grises).
fn rgb_to_ansi256(r: u8, g: u8, b: u8) -> u8 {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let nearest = |v: u8| -> usize {
        LEVELS
            .iter()
            .enumerate()
            .min_by_key(|(_, l)| (**l as i32 - v as i32).abs())
            .map(|(i, _)| i)
            .unwrap_or(0)
    };
    let (ri, gi, bi) = (nearest(r), nearest(g), nearest(b));
    let cube = (LEVELS[ri], LEVELS[gi], LEVELS[bi]);
    let cube_idx = 16 + 36 * ri + 6 * gi + bi;

    let avg = (r as u32 + g as u32 + b as u32) / 3;
    let gray_i = ((avg as i32 - 8).max(0) / 10).min(23) as u32;
    let gray_v = (8 + gray_i * 10) as u8;

    let d = |c: (u8, u8, u8)| -> i32 {
        let (dr, dg, db) = (c.0 as i32 - r as i32, c.1 as i32 - g as i32, c.2 as i32 - b as i32);
        dr * dr + dg * dg + db * db
    };
    if d((gray_v, gray_v, gray_v)) < d(cube) {
        (232 + gray_i) as u8
    } else {
        cube_idx as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_roundtrip() {
        for c in [Rgb(0.9, 0.2, 0.4), Rgb(0.1, 0.6, 0.3), Rgb(0.2, 0.3, 0.9), Rgb(0.5, 0.5, 0.5)] {
            let (h, s, l) = c.to_hsl();
            let back = Rgb::from_hsl(h, s, l);
            assert!(c.dist2(back) < 1e-4, "{c:?} -> {back:?}");
        }
    }

    #[test]
    fn ansi256_extremes() {
        assert_eq!(rgb_to_ansi256(0, 0, 0), 16);
        assert_eq!(rgb_to_ansi256(255, 255, 255), 231);
        assert_eq!(rgb_to_ansi256(255, 0, 0), 196);
    }
}

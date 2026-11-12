//! Paleta de la planta, extraída de la carátula del disco.

use crate::canvas::Canvas;
use crate::color::Rgb;

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// Pétalos exteriores: el color más vibrante de la carátula.
    pub petal: Rgb,
    /// Pétalos interiores: versión clara del anterior.
    pub petal_light: Rgb,
    /// Pistilo, polen y chispas: segundo color de la carátula.
    pub accent: Rgb,
    /// Tallo y zarcillos: versión oscura del color dominante.
    pub stem: Rgb,
    pub leaf: Rgb,
    pub bg_top: Rgb,
    pub bg_bottom: Rgb,
    pub soil: Rgb,
}

impl Default for Palette {
    /// Paleta fija verde/rosa, para archivos sin carátula.
    fn default() -> Self {
        let petal = Rgb(0.96, 0.42, 0.64);
        Palette {
            petal,
            petal_light: petal.mix(Rgb::WHITE, 0.4),
            accent: Rgb(1.0, 0.80, 0.34),
            stem: Rgb(0.20, 0.50, 0.30),
            leaf: Rgb(0.32, 0.68, 0.38),
            bg_top: Rgb(0.020, 0.030, 0.055),
            bg_bottom: Rgb(0.035, 0.060, 0.070),
            soil: Rgb(0.27, 0.19, 0.13),
        }
    }
}

impl Palette {
    pub fn mix(&self, o: &Palette, t: f32) -> Palette {
        Palette {
            petal: self.petal.mix(o.petal, t),
            petal_light: self.petal_light.mix(o.petal_light, t),
            accent: self.accent.mix(o.accent, t),
            stem: self.stem.mix(o.stem, t),
            leaf: self.leaf.mix(o.leaf, t),
            bg_top: self.bg_top.mix(o.bg_top, t),
            bg_bottom: self.bg_bottom.mix(o.bg_bottom, t),
            soil: self.soil.mix(o.soil, t),
        }
    }

    /// Decodifica una imagen (JPEG o PNG) y arma la paleta. `None` si no se puede leer.
    pub fn from_image_bytes(bytes: &[u8]) -> Option<Palette> {
        let img = image::load_from_memory(bytes).ok()?;
        let thumb = img.thumbnail(48, 48).to_rgb8();
        let pixels: Vec<Rgb> = thumb.pixels().map(|p| Rgb::from_u8(p[0], p[1], p[2])).collect();
        Some(Self::from_pixels(&pixels))
    }

    /// K-means pequeño sobre los píxeles y elección de roles por tono y saturación.
    pub fn from_pixels(pixels: &[Rgb]) -> Palette {
        if pixels.is_empty() {
            return Palette::default();
        }
        let clusters = kmeans(pixels, 6, 10);
        let total: f32 = clusters.iter().map(|c| c.1 as f32).sum();

        // Dominante = grupo con más píxeles.
        let dominant = clusters
            .iter()
            .max_by_key(|c| c.1)
            .map(|c| c.0)
            .unwrap_or(Rgb(0.5, 0.5, 0.5));
        let (dh, ds, _) = dominant.to_hsl();

        // Qué tan "vibrante" es un grupo: saturado, ni muy oscuro ni muy claro, y con presencia.
        let vibrancy = |c: &(Rgb, usize)| -> f32 {
            let (_, s, l) = c.0.to_hsl();
            let light = (1.0 - (l - 0.55).abs() * 1.6).max(0.05);
            s * light * (c.1 as f32 / total).powf(0.3)
        };
        let best = clusters
            .iter()
            .max_by(|a, b| vibrancy(a).total_cmp(&vibrancy(b)))
            .copied()
            .unwrap_or((dominant, 1));
        let (ph, ps, pl) = best.0.to_hsl();

        // Carátula casi gris: flor clara con pistilo cálido.
        let grayscale = ps < 0.12;
        let petal = if grayscale {
            Rgb(0.90, 0.90, 0.93)
        } else {
            Rgb::from_hsl(ph, ps.max(0.55), pl.clamp(0.52, 0.68))
        };

        // Acento: el grupo vibrante cuyo tono está más lejos del de los pétalos.
        let hue_dist = |a: f32, b: f32| {
            let d = (a - b).abs();
            d.min(1.0 - d)
        };
        let accent = clusters
            .iter()
            .filter(|c| {
                let (h, s, _) = c.0.to_hsl();
                s > 0.2 && hue_dist(h, ph) > 0.11
            })
            .max_by(|a, b| {
                let score = |c: &(Rgb, usize)| hue_dist(c.0.to_hsl().0, ph) * vibrancy(c);
                score(a).total_cmp(&score(b))
            })
            .map(|c| {
                let (h, s, l) = c.0.to_hsl();
                Rgb::from_hsl(h, s.max(0.6), l.clamp(0.60, 0.78))
            })
            .unwrap_or_else(|| {
                if grayscale {
                    Rgb(1.0, 0.80, 0.36)
                } else {
                    // Sin segundo color: un análogo más claro del tono de los pétalos.
                    Rgb::from_hsl(ph + 0.12, 0.85, 0.72)
                }
            });

        // Tallo: tono del dominante, oscuro pero visible sobre el fondo.
        let (stem_h, stem_s) = if ds >= 0.12 {
            (dh, ds.clamp(0.35, 0.60))
        } else if !grayscale {
            (ph, 0.40)
        } else {
            (0.38, 0.35) // verde apagado
        };
        let bg_s = if ds >= 0.12 || !grayscale { 0.35 } else { 0.08 };

        Palette {
            petal,
            petal_light: petal.mix(Rgb::WHITE, 0.4),
            accent,
            stem: Rgb::from_hsl(stem_h, stem_s, 0.36),
            leaf: Rgb::from_hsl(stem_h, stem_s, 0.48),
            bg_top: Rgb::from_hsl(stem_h, bg_s, 0.040),
            bg_bottom: Rgb::from_hsl(stem_h, bg_s, 0.075),
            soil: Rgb::from_hsl(stem_h, bg_s * 0.7, 0.21),
        }
    }
}

/// K-means determinista. Devuelve (centro, cantidad de píxeles) por grupo no vacío.
fn kmeans(pixels: &[Rgb], k: usize, iters: usize) -> Vec<(Rgb, usize)> {
    // Semillas: píxeles repartidos a lo largo del orden por luminosidad.
    let mut sorted: Vec<Rgb> = pixels.to_vec();
    sorted.sort_by(|a, b| luma(*a).total_cmp(&luma(*b)));
    let mut centers: Vec<Rgb> = (0..k)
        .map(|i| sorted[(i * 2 + 1) * sorted.len() / (k * 2)])
        .collect();
    let mut counts = vec![0usize; k];

    for _ in 0..iters {
        let mut sums = vec![(0.0f32, 0.0f32, 0.0f32); k];
        counts.iter_mut().for_each(|c| *c = 0);
        for p in pixels {
            let i = (0..k)
                .min_by(|a, b| p.dist2(centers[*a]).total_cmp(&p.dist2(centers[*b])))
                .unwrap_or(0);
            sums[i].0 += p.0;
            sums[i].1 += p.1;
            sums[i].2 += p.2;
            counts[i] += 1;
        }
        for i in 0..k {
            if counts[i] > 0 {
                let n = counts[i] as f32;
                centers[i] = Rgb(sums[i].0 / n, sums[i].1 / n, sums[i].2 / n);
            }
        }
    }
    centers
        .into_iter()
        .zip(counts)
        .filter(|(_, n)| *n > 0)
        .collect()
}

fn luma(c: Rgb) -> f32 {
    0.2126 * c.0 + 0.7152 * c.1 + 0.0722 * c.2
}

/// Miniatura cuadrada de la carátula, para mostrarla en pantalla.
#[derive(Clone, Debug)]
pub struct Cover {
    side: usize,
    px: Vec<Rgb>,
}

impl Cover {
    const SIDE: u32 = 48;

    /// Decodifica la imagen y la recorta al centro. `None` si no se puede leer.
    pub fn from_image_bytes(bytes: &[u8]) -> Option<Cover> {
        let img = image::load_from_memory(bytes).ok()?;
        let thumb = img.resize_to_fill(Self::SIDE, Self::SIDE, image::imageops::FilterType::Triangle).to_rgb8();
        let px = thumb.pixels().map(|p| Rgb::from_u8(p[0], p[1], p[2])).collect();
        Some(Cover { side: Self::SIDE as usize, px })
    }

    /// Color medio del bloque de la miniatura que cae en la celda (`i`, `j`) de una
    /// cuadrícula de `n`×`n`.
    fn sample(&self, i: usize, j: usize, n: usize) -> Rgb {
        let range = |k: usize| (k * self.side / n, ((k + 1) * self.side / n).max(k * self.side / n + 1).min(self.side));
        let ((x0, x1), (y0, y1)) = (range(i), range(j));
        let (mut sum, mut count) = ((0.0, 0.0, 0.0), 0.0f32);
        for y in y0..y1 {
            for x in x0..x1 {
                let c = self.px[y * self.side + x];
                sum = (sum.0 + c.0, sum.1 + c.1, sum.2 + c.2);
                count += 1.0;
            }
        }
        Rgb(sum.0 / count.max(1.0), sum.1 / count.max(1.0), sum.2 / count.max(1.0))
    }

    /// Dibuja la carátula abajo a la izquierda, con un marco fino. `bottom_margin`: píxeles
    /// libres bajo ella. No dibuja nada si el lienzo es demasiado chico.
    pub fn draw(&self, canvas: &mut Canvas, bottom_margin: usize, frame: Rgb) {
        let (w, h) = (canvas.w, canvas.px_h());
        let side = ((h as f32 * 0.30) as usize).min(w / 5).min(self.side);
        if side < 8 || h < side + bottom_margin + 4 {
            return;
        }
        let (x0, y0) = (2i32, (h - bottom_margin - side - 1) as i32);
        for j in -1..=side as i32 {
            for i in -1..=side as i32 {
                let edge = i < 0 || j < 0 || i == side as i32 || j == side as i32;
                let color = if edge { frame.scale(0.55) } else { self.sample(i as usize, j as usize, side) };
                canvas.blend(x0 + i + 1, y0 + j, color, 1.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_vibrant_petal_over_dull_dominant() {
        // 80% azul oscuro apagado, 20% naranja intenso.
        let mut px = vec![Rgb(0.10, 0.12, 0.22); 800];
        px.extend(vec![Rgb(0.95, 0.50, 0.10); 200]);
        let p = Palette::from_pixels(&px);
        let (h, s, _) = p.petal.to_hsl();
        assert!(s > 0.5, "pétalo saturado, s={s}");
        assert!((h - 0.078).abs() < 0.05, "tono naranja, h={h}");
        // El tallo toma el tono del dominante (azul).
        let (sh, _, sl) = p.stem.to_hsl();
        assert!((sh - 0.64).abs() < 0.06, "tallo azul, h={sh}");
        assert!(sl > 0.3, "tallo visible sobre el fondo");
    }

    #[test]
    fn grayscale_cover_gets_light_flower() {
        let px: Vec<Rgb> = (0..500).map(|i| {
            let v = i as f32 / 500.0;
            Rgb(v, v, v)
        }).collect();
        let p = Palette::from_pixels(&px);
        assert!(luma(p.petal) > 0.8);
    }

    #[test]
    fn cover_thumbnail_keeps_the_picture() {
        // Mitad izquierda roja, mitad derecha azul.
        let mut img = image::RgbImage::new(64, 32);
        for (x, _, p) in img.enumerate_pixels_mut() {
            *p = if x < 32 { image::Rgb([220, 20, 20]) } else { image::Rgb([20, 20, 220]) };
        }
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        let cover = Cover::from_image_bytes(&bytes).expect("miniatura");
        let (left, right) = (cover.sample(1, 5, 12), cover.sample(10, 5, 12));
        assert!(left.0 > 0.7 && left.2 < 0.2, "{left:?}");
        assert!(right.2 > 0.7 && right.0 < 0.2, "{right:?}");

        // Lienzo grande: se dibuja. Lienzo mínimo: no hace nada ni entra en pánico.
        let mut canvas = Canvas::new(120, 40);
        canvas.clear(|_, _| Rgb::BLACK);
        cover.draw(&mut canvas, 4, Rgb::WHITE);
        assert!(canvas.covered(2, 30) || canvas.covered(4, 28));
        cover.draw(&mut Canvas::new(4, 2), 4, Rgb::WHITE);
    }
}

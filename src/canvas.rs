//! Lienzo de tres capas sobre la grilla de la terminal.
//!
//! 1. Píxeles: 1×2 por celda, dibujados con `▀` (color de frente = píxel superior, fondo =
//!    inferior). Color completo por píxel: fondo, resplandor y pétalos.
//! 2. Braille: 2×4 puntos por celda. Trazo fino de un solo color: tallo, hojas, zarcillos.
//! 3. Glifos: un carácter por celda: polen, chispas, pétalos que caen, tierra.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::color::{ColorMode, Rgb};

pub struct Canvas {
    /// Ancho y alto en celdas.
    pub w: usize,
    pub h: usize,
    px: Vec<Rgb>,
    /// Cobertura opaca por píxel (0..1). Si es alta, la celda muestra píxeles y no Braille.
    cover: Vec<f32>,
    dots: Vec<u8>,
    dot_color: Vec<(f32, f32, f32, f32)>,
    glyphs: Vec<Option<(char, Rgb)>>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Self {
        let mut c = Canvas {
            w: 0,
            h: 0,
            px: Vec::new(),
            cover: Vec::new(),
            dots: Vec::new(),
            dot_color: Vec::new(),
            glyphs: Vec::new(),
        };
        c.resize(w, h);
        c
    }

    pub fn resize(&mut self, w: usize, h: usize) {
        if w == self.w && h == self.h {
            return;
        }
        self.w = w;
        self.h = h;
        self.px = vec![Rgb::BLACK; w * h * 2];
        self.cover = vec![0.0; w * h * 2];
        self.dots = vec![0; w * h];
        self.dot_color = vec![(0.0, 0.0, 0.0, 0.0); w * h];
        self.glyphs = vec![None; w * h];
    }

    /// Alto de la capa de píxeles.
    pub fn px_h(&self) -> usize {
        self.h * 2
    }

    /// Limpia las tres capas y pinta el fondo con `f(x, y)` por píxel.
    pub fn clear(&mut self, f: impl Fn(usize, usize) -> Rgb) {
        for y in 0..self.h * 2 {
            for x in 0..self.w {
                self.px[y * self.w + x] = f(x, y);
            }
        }
        self.cover.iter_mut().for_each(|c| *c = 0.0);
        self.dots.iter_mut().for_each(|d| *d = 0);
        self.dot_color.iter_mut().for_each(|d| *d = (0.0, 0.0, 0.0, 0.0));
        self.glyphs.iter_mut().for_each(|g| *g = None);
    }

    fn px_index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h * 2 {
            None
        } else {
            Some(y as usize * self.w + x as usize)
        }
    }

    /// Mezcla un color opaco con transparencia `alpha` sobre el píxel.
    pub fn blend(&mut self, x: i32, y: i32, c: Rgb, alpha: f32) {
        if let Some(i) = self.px_index(x, y) {
            self.px[i] = self.px[i].mix(c, alpha);
            self.cover[i] = self.cover[i].max(alpha);
        }
    }

    /// Pinta el píxel con transparencia, como parte del fondo: no cuenta como cobertura
    /// opaca, así que el trazo Braille se sigue viendo encima (cerros, luna).
    pub fn paint(&mut self, x: i32, y: i32, c: Rgb, alpha: f32) {
        if let Some(i) = self.px_index(x, y) {
            self.px[i] = self.px[i].mix(c, alpha);
        }
    }

    /// Suma luz al píxel (resplandor). No cuenta como cobertura opaca.
    pub fn glow(&mut self, x: i32, y: i32, c: Rgb, k: f32) {
        if let Some(i) = self.px_index(x, y) {
            self.px[i] = self.px[i].add(c, k);
        }
    }

    /// Enciende un punto Braille. Coordenadas en puntos: 2 por celda en x, 4 en y.
    pub fn dot(&mut self, x: i32, y: i32, c: Rgb) {
        if x < 0 || y < 0 || x as usize >= self.w * 2 || y as usize >= self.h * 4 {
            return;
        }
        let (cx, cy) = (x as usize / 2, y as usize / 4);
        const BITS: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];
        let i = cy * self.w + cx;
        self.dots[i] |= BITS[y as usize % 4][x as usize % 2];
        let d = &mut self.dot_color[i];
        *d = (d.0 + c.0, d.1 + c.1, d.2 + c.2, d.3 + 1.0);
    }

    /// Punto Braille translúcido: `alpha` lo funde con lo que haya detrás en la capa de
    /// píxeles (resplandor, nebulosa), para que un trazo tenue no se vea oscuro sobre luz.
    pub fn dot_over(&mut self, x: i32, y: i32, c: Rgb, alpha: f32) {
        if x < 0 || y < 0 || x as usize >= self.w * 2 || y as usize >= self.h * 4 {
            return;
        }
        let behind = self.cell_bg(x as usize / 2, y as usize / 4);
        self.dot(x, y, behind.mix(c, alpha));
    }

    /// Línea Braille entre dos puntos (coordenadas en puntos, con decimales).
    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Rgb) {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).ceil().max(1.0) as i32;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            self.dot(
                (x0 + (x1 - x0) * t).round() as i32,
                (y0 + (y1 - y0) * t).round() as i32,
                c,
            );
        }
    }

    /// Color medio del fondo de una celda (sus dos píxeles).
    fn cell_bg(&self, cx: usize, cy: usize) -> Rgb {
        let top = self.px[cy * 2 * self.w + cx];
        let bot = self.px[(cy * 2 + 1) * self.w + cx];
        top.mix(bot, 0.5)
    }

    /// Coloca un glifo en una celda. `alpha` lo funde con el fondo.
    pub fn glyph(&mut self, cx: i32, cy: i32, ch: char, c: Rgb, alpha: f32) {
        if cx < 0 || cy < 0 || cx as usize >= self.w || cy as usize >= self.h {
            return;
        }
        let (cx, cy) = (cx as usize, cy as usize);
        let color = self.cell_bg(cx, cy).mix(c, alpha);
        self.glyphs[cy * self.w + cx] = Some((ch, color));
    }

    /// ¿La celda tiene píxeles opacos (flor, sol, planeta)?
    pub fn covered(&self, cx: i32, cy: i32) -> bool {
        if cx < 0 || cy < 0 || cx as usize >= self.w || cy as usize >= self.h {
            return false;
        }
        let (cx, cy) = (cx as usize, cy as usize);
        self.cover[cy * 2 * self.w + cx].max(self.cover[(cy * 2 + 1) * self.w + cx]) > 0.3
    }

    /// ¿La celda ya tiene algo dibujado encima del fondo (píxeles opacos, trazo o glifo)?
    pub fn occupied(&self, cx: i32, cy: i32) -> bool {
        if cx < 0 || cy < 0 || cx as usize >= self.w || cy as usize >= self.h {
            return true;
        }
        let i = cy as usize * self.w + cx as usize;
        self.covered(cx, cy) || self.dots[i] != 0 || self.glyphs[i].is_some()
    }

    /// Convierte el lienzo en una imagen, para la postal. Cada celda mide 8×16 píxeles.
    /// Los píxeles y el Braille salen exactos; los glifos, que son texto, se aproximan con
    /// puntos y bloques.
    pub fn to_image(&self) -> image::RgbImage {
        const CW: u32 = 8;
        const CH: u32 = 16;
        let mut img = image::RgbImage::new(self.w as u32 * CW, self.h as u32 * CH);
        let rgb = |c: Rgb| {
            let (r, g, b) = c.to_u8();
            image::Rgb([r, g, b])
        };
        let fill = |img: &mut image::RgbImage, x0: u32, y0: u32, w: u32, h: u32, c: Rgb| {
            for y in y0..(y0 + h).min(img.height()) {
                for x in x0..(x0 + w).min(img.width()) {
                    img.put_pixel(x, y, rgb(c));
                }
            }
        };
        for cy in 0..self.h {
            for cx in 0..self.w {
                let i = cy * self.w + cx;
                let (x0, y0) = (cx as u32 * CW, cy as u32 * CH);
                let top = self.px[cy * 2 * self.w + cx];
                let bot = self.px[(cy * 2 + 1) * self.w + cx];
                fill(&mut img, x0, y0, CW, CH / 2, top);
                fill(&mut img, x0, y0 + CH / 2, CW, CH / 2, bot);
                let solid = self.cover[cy * 2 * self.w + cx].max(self.cover[(cy * 2 + 1) * self.w + cx]);
                if let Some((ch, color)) = self.glyphs[i] {
                    let bg = top.mix(bot, 0.5);
                    match ch {
                        '░' => fill(&mut img, x0, y0, CW, CH, bg.mix(color, 0.25)),
                        '▒' => fill(&mut img, x0, y0, CW, CH, bg.mix(color, 0.5)),
                        '▓' => fill(&mut img, x0, y0, CW, CH, bg.mix(color, 0.75)),
                        '█' => fill(&mut img, x0, y0, CW, CH, color),
                        // Flores y estrellas: una cruz. El resto: un punto.
                        '✿' | '❀' | '❁' | '✾' | '✦' | '✧' | '⁕' => {
                            fill(&mut img, x0 + 3, y0 + 4, 2, 8, color);
                            fill(&mut img, x0 + 1, y0 + 7, 6, 2, color);
                        }
                        '•' => fill(&mut img, x0 + 2, y0 + 6, 4, 4, color),
                        _ => fill(&mut img, x0 + 3, y0 + 7, 2, 2, color),
                    }
                } else if self.dots[i] != 0 && solid < 0.5 {
                    let d = self.dot_color[i];
                    let color = Rgb(d.0 / d.3, d.1 / d.3, d.2 / d.3);
                    const BITS: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];
                    for (row, bits) in BITS.iter().enumerate() {
                        for (col, bit) in bits.iter().enumerate() {
                            if self.dots[i] & bit != 0 {
                                fill(&mut img, x0 + col as u32 * 4, y0 + row as u32 * 4, 3, 3, color);
                            }
                        }
                    }
                }
            }
        }
        img
    }

    /// Vuelca el lienzo en el buffer de ratatui.
    pub fn render(&self, buf: &mut Buffer, area: Rect, mode: ColorMode) {
        let w = self.w.min(area.width as usize);
        let h = self.h.min(area.height as usize);
        for cy in 0..h {
            for cx in 0..w {
                let i = cy * self.w + cx;
                let top = self.px[cy * 2 * self.w + cx];
                let bot = self.px[(cy * 2 + 1) * self.w + cx];
                let solid = self.cover[cy * 2 * self.w + cx].max(self.cover[(cy * 2 + 1) * self.w + cx]);
                let cell = &mut buf[(area.x + cx as u16, area.y + cy as u16)];

                if let Some((ch, color)) = self.glyphs[i] {
                    cell.set_char(ch).set_style(
                        Style::new().fg(mode.color(color)).bg(mode.color(top.mix(bot, 0.5))),
                    );
                } else if self.dots[i] != 0 && solid < 0.5 {
                    let d = self.dot_color[i];
                    let fg = Rgb(d.0 / d.3, d.1 / d.3, d.2 / d.3);
                    let ch = char::from_u32(0x2800 + self.dots[i] as u32).unwrap_or(' ');
                    cell.set_char(ch)
                        .set_style(Style::new().fg(mode.color(fg)).bg(mode.color(top.mix(bot, 0.5))));
                } else {
                    let (ct, cb) = (mode.color(top), mode.color(bot));
                    if ct == cb {
                        cell.set_char(' ').set_style(Style::new().bg(cb));
                    } else {
                        cell.set_char('▀').set_style(Style::new().fg(ct).bg(cb));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braille_bits_map_to_unicode() {
        let mut c = Canvas::new(2, 1);
        c.clear(|_, _| Rgb::BLACK);
        c.dot(0, 0, Rgb::WHITE); // arriba-izquierda: bit 1
        c.dot(1, 3, Rgb::WHITE); // abajo-derecha: bit 8
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
        c.render(&mut buf, Rect::new(0, 0, 2, 1), ColorMode::True);
        assert_eq!(buf[(0, 0)].symbol(), "⢁");
    }

    #[test]
    fn image_export_matches_the_canvas() {
        let mut c = Canvas::new(3, 2);
        c.clear(|_, _| Rgb(0.0, 0.0, 1.0));
        c.blend(0, 0, Rgb(1.0, 0.0, 0.0), 1.0); // píxel superior de la celda (0,0)
        c.dot(2, 0, Rgb(0.0, 1.0, 0.0)); // punto Braille en la celda (1,0)
        let img = c.to_image();
        assert_eq!((img.width(), img.height()), (24, 32));
        assert_eq!(img.get_pixel(3, 3).0, [255, 0, 0]);
        assert_eq!(img.get_pixel(3, 12).0, [0, 0, 255]);
        assert_eq!(img.get_pixel(9, 1).0, [0, 255, 0]);
        assert_eq!(img.get_pixel(11, 3).0, [0, 0, 255], "fuera del punto de 3×3");
        assert_eq!(img.get_pixel(20, 20).0, [0, 0, 255]);
    }

    #[test]
    fn opaque_pixels_hide_braille() {
        let mut c = Canvas::new(1, 1);
        c.clear(|_, _| Rgb::BLACK);
        c.dot(0, 0, Rgb::WHITE);
        c.blend(0, 0, Rgb(1.0, 0.0, 0.0), 1.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        c.render(&mut buf, Rect::new(0, 0, 1, 1), ColorMode::True);
        assert_eq!(buf[(0, 0)].symbol(), "▀");
    }
}

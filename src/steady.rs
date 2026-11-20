//! Menos datos a la terminal: si el color de una celda cambió tan poco que no se nota, se
//! deja el de antes.
//!
//! Las escenas se mueven todo el tiempo, pero gran parte del cambio cuadro a cuadro es
//! imperceptible: el cielo que se aclara de a una unidad, un brillo que late apenas. Cada
//! una de esas celdas obliga a la terminal a recibir y redibujar sus colores. Como ratatui
//! solo manda las celdas distintas al cuadro anterior, basta con devolverles el color
//! anterior para que no viajen. El umbral crece con el brillo: en lo oscuro, donde un paso
//! se nota más, es más estricto.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Lo que se mandó la última vez en cada celda: su primer carácter y sus colores.
type Shown = (char, [u8; 3], [u8; 3]);

#[derive(Default)]
pub struct Steady {
    area: Rect,
    cells: Vec<Option<Shown>>,
}

fn rgb(c: Color) -> Option<[u8; 3]> {
    match c {
        Color::Rgb(r, g, b) => Some([r, g, b]),
        _ => None,
    }
}

/// ¿Se notaría pasar de `a` a `b`?
fn unnoticed(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(&x, y)| x.abs_diff(y) <= 1 + x.max(y) / 48)
}

impl Steady {
    /// Ajusta el cuadro recién dibujado en `buf`, dentro de `area`.
    pub fn apply(&mut self, buf: &mut Buffer, area: Rect) {
        if area != self.area {
            self.area = area;
            self.cells = vec![None; area.width as usize * area.height as usize];
        }
        for y in 0..area.height {
            for x in 0..area.width {
                let cell = &mut buf[(area.x + x, area.y + y)];
                let slot = &mut self.cells[y as usize * area.width as usize + x as usize];
                let (Some(fg), Some(bg)) = (rgb(cell.fg), rgb(cell.bg)) else {
                    *slot = None;
                    continue;
                };
                let ch = cell.symbol().chars().next().unwrap_or(' ');
                match slot {
                    Some((old_ch, old_fg, old_bg)) if *old_ch == ch && unnoticed(*old_bg, bg) && (ch == ' ' || unnoticed(*old_fg, fg)) => {
                        cell.fg = Color::Rgb(old_fg[0], old_fg[1], old_fg[2]);
                        cell.bg = Color::Rgb(old_bg[0], old_bg[1], old_bg[2]);
                    }
                    _ => *slot = Some((ch, fg, bg)),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(buf: &mut Buffer, ch: &str, fg: [u8; 3], bg: [u8; 3]) {
        buf[(0, 0)].set_symbol(ch).set_fg(Color::Rgb(fg[0], fg[1], fg[2])).set_bg(Color::Rgb(bg[0], bg[1], bg[2]));
    }

    #[test]
    fn small_changes_keep_the_old_color_big_ones_pass() {
        let area = Rect::new(0, 0, 1, 1);
        let mut steady = Steady::default();
        let mut buf = Buffer::empty(area);
        cell(&mut buf, "▀", [100, 100, 100], [20, 20, 20]);
        steady.apply(&mut buf, area);
        // Un paso chico: se queda como estaba.
        cell(&mut buf, "▀", [102, 101, 100], [21, 20, 20]);
        steady.apply(&mut buf, area);
        assert_eq!(buf[(0, 0)].fg, Color::Rgb(100, 100, 100));
        assert_eq!(buf[(0, 0)].bg, Color::Rgb(20, 20, 20));
        // Muchos pasos chicos se acumulan contra el color guardado, no contra el anterior.
        cell(&mut buf, "▀", [100, 100, 100], [24, 20, 20]);
        steady.apply(&mut buf, area);
        assert_eq!(buf[(0, 0)].bg, Color::Rgb(24, 20, 20));
        // Otro carácter: pasa siempre.
        cell(&mut buf, "⠁", [100, 100, 100], [24, 20, 20]);
        steady.apply(&mut buf, area);
        assert_eq!(buf[(0, 0)].symbol(), "⠁");
    }

    #[test]
    fn dark_colors_are_stricter_than_bright_ones() {
        assert!(unnoticed([10, 10, 10], [11, 11, 11]));
        assert!(!unnoticed([10, 10, 10], [13, 10, 10]));
        assert!(unnoticed([240, 240, 240], [245, 240, 240]));
        assert!(!unnoticed([240, 240, 240], [250, 240, 240]));
    }

    #[test]
    fn spaces_ignore_their_invisible_foreground() {
        let area = Rect::new(0, 0, 1, 1);
        let mut steady = Steady::default();
        let mut buf = Buffer::empty(area);
        cell(&mut buf, " ", [0, 0, 0], [50, 50, 50]);
        steady.apply(&mut buf, area);
        cell(&mut buf, " ", [200, 0, 0], [50, 50, 51]);
        steady.apply(&mut buf, area);
        assert_eq!(buf[(0, 0)].bg, Color::Rgb(50, 50, 50));
    }
}

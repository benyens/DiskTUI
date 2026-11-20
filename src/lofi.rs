//! Escena "cuarto lofi": un escritorio frente a una ventana, con un gato en el alféizar.
//!
//! Afuera está el cielo y el clima de la canción ([`Scenery`]); si llueve, las gotas
//! corren por el vidrio. Adentro, la lámpara late con el pulso, el tocadiscos gira con el
//! tempo, el gato mueve la cola al compás y levanta las orejas con los platillos, y la
//! guirnalda de luces titila con los agudos. En los clímax se encienden todas las luces.

use std::f32::consts::TAU;

use crate::analysis::Bands;
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::palette::{Cover, Palette};
use crate::plant::{Rng, smoothstep};
use crate::render::View;
use crate::scene::{Frame, Scene};
use crate::scenery::Scenery;

/// Ventana, como fracciones del lienzo: izquierda, arriba, derecha, abajo.
/// Dónde está el tocadiscos, como fracción del ancho.
const TURNTABLE_X: f32 = 0.78;
const WINDOW: (f32, f32, f32, f32) = (0.12, 0.10, 0.62, 0.62);
/// Altura del escritorio.
const DESK: f32 = 0.74;
const MAX_DROPS: usize = 140;
const MAX_NOTES: usize = 12;

struct Drop {
    /// Posición dentro del vidrio, en fracciones de la ventana.
    x: f32,
    y: f32,
    speed: f32,
}

/// Nota musical que sube y se desvanece.
struct Note {
    x: f32,
    y: f32,
    age: f32,
}

pub struct Lofi {
    pub scenery: Scenery,
    rng: Rng,
    time: f32,
    progress: f32,
    bands: Bands,
    beat_pulse: f32,
    hat_pulse: f32,
    climax_pulse: f32,
    /// Giro del disco, en vueltas.
    record: f32,
    drops: Vec<Drop>,
    notes: Vec<Note>,
    /// Canción nueva: la lámpara parpadea y el disco se cambia.
    switching: Option<f32>,
    /// Carátula de lo que suena: el disco es un picture disc con ella y su funda queda
    /// apoyada al lado.
    pub cover: Option<Cover>,
}

impl Lofi {
    pub fn new(seed: u64) -> Self {
        let mut scenery = Scenery::new(seed ^ 0x10F1_0001);
        scenery.bare = true;
        Lofi {
            scenery,
            rng: Rng::new(seed ^ 0x0CA7_CA75),
            time: 0.0,
            progress: 0.0,
            bands: Bands::default(),
            beat_pulse: 0.0,
            hat_pulse: 0.0,
            climax_pulse: 0.0,
            record: 0.0,
            drops: Vec::new(),
            notes: Vec::new(),
            switching: None,
            cover: None,
        }
    }

    fn presence(&self) -> f32 {
        self.switching.map(|t| 1.0 - smoothstep(0.0, 0.5, t.min(1.0 - t))).unwrap_or(1.0)
    }

    /// Fase del pulso para la cola del gato: la del análisis si hay tempo, si no a 80 BPM.
    fn phase(&self) -> f32 {
        if self.bands.song.bpm > 0.0 && self.bands.song.tempo_confidence > 0.2 {
            self.bands.beat_phase
        } else {
            (self.time * 80.0 / 60.0).fract()
        }
    }

    fn advance(&mut self, f: &Frame) {
        let (dt, b) = (f.dt, f.bands);
        self.time += dt;
        self.bands = *b;
        let decay = |v: f32, tau: f32, hit: bool| (v * (-dt / tau).exp()).max(if hit { 1.0 } else { 0.0 });
        self.beat_pulse = decay(self.beat_pulse, 0.30, b.beat);
        self.hat_pulse = decay(self.hat_pulse, 0.25, b.hat);
        self.climax_pulse = decay(self.climax_pulse, 2.5, b.climax);
        if f.playing {
            // 33⅓ RPM, pero que se note: una vuelta cada ~1,8 s.
            self.record += dt * 0.55;
        }
        if let Some(t) = &mut self.switching {
            *t += dt / 1.2;
            if *t >= 1.0 {
                self.switching = None;
                self.progress = 0.0;
            }
        } else if let Some(p) = f.progress {
            self.progress = p;
        } else if f.playing {
            self.progress = (self.progress + dt / f.song_len.max(1.0)).min(1.0);
        }

        // Gotas en el vidrio, según cuánto llueve afuera.
        let rain = self.scenery.atmosphere().rain;
        let rate = rain * 25.0 * dt;
        let count = rate as usize + usize::from(self.rng.next() < rate.fract());
        for _ in 0..count.min(MAX_DROPS.saturating_sub(self.drops.len())) {
            self.drops.push(Drop { x: self.rng.next(), y: self.rng.range(0.0, 0.6), speed: self.rng.range(0.02, 0.30) });
        }
        for d in &mut self.drops {
            // Bajan a tirones: se quedan pegadas y de pronto resbalan.
            let slip = if (d.x * 97.0 + self.time * 0.8).sin() > 0.6 { 4.0 } else { 0.4 };
            d.y += d.speed * slip * dt;
        }
        self.drops.retain(|d| d.y < 1.0);

        // Notas que salen del tocadiscos con los pulsos fuertes.
        if f.playing && b.beat && b.intensity > 0.55 && self.rng.next() < 0.5 && self.notes.len() < MAX_NOTES {
            let x = TURNTABLE_X + self.rng.range(-0.03, 0.03);
            self.notes.push(Note { x, y: DESK + 0.02, age: 0.0 });
        }
        for n in &mut self.notes {
            n.age += dt;
            n.y -= dt * 0.05;
            n.x += dt * 0.012 * (n.age * 3.0).sin();
        }
        self.notes.retain(|n| n.age < 3.0);

        self.scenery.update(dt, b, Some(self.progress), f.playing, 1.0, &[]);
    }

    fn draw_room(&self, canvas: &mut Canvas, pal: &Palette) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let atm = self.scenery.atmosphere();
        let day = atm.daylight;
        let (wl, wt, wr, wb) = (WINDOW.0 * w, WINDOW.1 * h, WINDOW.2 * w, WINDOW.3 * h);
        let lamp_x = w * 0.90;
        let lamp_y = h * (DESK - 0.18);
        let lamp_on = self.presence() * (0.75 + 0.25 * self.beat_pulse + 0.4 * self.climax_pulse);
        let warm = Rgb(1.0, 0.70, 0.40).mix(pal.accent, 0.2);
        let wall = Rgb(0.10, 0.08, 0.10).mix(pal.bg_bottom, 0.25).mix(Rgb(0.42, 0.36, 0.34), day * 0.5);
        let wood = Rgb(0.20, 0.12, 0.08).mix(Rgb(0.45, 0.30, 0.20), day * 0.5);
        let frame_color = Rgb(0.85, 0.82, 0.76).scale(0.30 + 0.45 * day);
        let desk_y = h * DESK;
        for y in 0..h as i32 {
            for x in 0..canvas.w as i32 {
                let (fx, fy) = (x as f32, y as f32);
                let inside = fx > wl && fx < wr && fy > wt && fy < wb;
                // Cruz del marco de la ventana.
                let mullion = inside && ((fx - (wl + wr) / 2.0).abs() < 0.8 || (fy - (wt + wb) / 2.0).abs() < 0.5);
                if inside && !mullion {
                    continue;
                }
                let border = !inside && fx > wl - 1.5 && fx < wr + 1.5 && fy > wt - 1.5 && fy < wb + 1.5;
                // Luz de la lámpara sobre la pared y el escritorio.
                let d = (((fx - lamp_x) / 2.0).powi(2) + (fy - lamp_y).powi(2)).sqrt() / (h * 0.7);
                let lit = lamp_on * 0.35 * (1.0 - d).max(0.0).powf(1.5);
                let color = if mullion || border {
                    frame_color.add(warm, lit * 0.5)
                } else if fy >= desk_y {
                    let edge = fy < desk_y + 1.0;
                    wood.scale(if edge { 1.25 } else { 1.0 - 0.25 * (fy - desk_y) / (h - desk_y).max(1.0) }).add(warm, lit)
                } else {
                    wall.add(warm, lit)
                };
                canvas.solid(x, y, color);
            }
        }
        // Alféizar bajo la ventana.
        for x in (wl - 3.0) as i32..=(wr + 3.0) as i32 {
            canvas.solid(x, wb as i32 + 1, frame_color.scale(1.1));
        }
        self.draw_lights(canvas, wl, wr, wt, pal);
        self.draw_lamp(canvas, lamp_x, lamp_y, desk_y, lamp_on, warm);
        self.draw_desk_things(canvas, desk_y, pal, warm, lamp_on);
        self.draw_cat(canvas, wl + (wr - wl) * 0.70, wb, warm, lamp_on);
    }

    /// Guirnalda de luces sobre la ventana: titila con los agudos, se enciende en el clímax.
    fn draw_lights(&self, canvas: &mut Canvas, wl: f32, wr: f32, wt: f32, pal: &Palette) {
        let n = 12;
        let colors = [pal.petal, pal.accent, Rgb(1.0, 0.85, 0.5), pal.petal_light];
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let x = wl - 2.0 + (wr - wl + 4.0) * t;
            let y = wt - 2.0 + 2.5 * (t * TAU / 2.0).sin().abs();
            let flicker = (self.time * (2.0 + i as f32 * 0.37) + i as f32).sin() * 0.5 + 0.5;
            let on = self.presence() * (0.35 + 0.35 * flicker * (0.4 + self.bands.treble) + 0.6 * self.climax_pulse).min(1.0);
            let c = colors[i % 4].mix(Rgb::WHITE, 0.2);
            canvas.solid(x as i32, y as i32, c.scale(0.4 + 0.6 * on));
            canvas.glow(x as i32, y as i32 + 1, c, 0.15 * on);
        }
    }

    fn draw_lamp(&self, canvas: &mut Canvas, x: f32, y: f32, desk: f32, on: f32, warm: Rgb) {
        let metal = Rgb(0.12, 0.12, 0.14);
        for j in y as i32..desk as i32 {
            canvas.solid(x as i32, j, metal);
        }
        for dx in -2..=2 {
            canvas.solid(x as i32 + dx, desk as i32 - 1, metal);
        }
        // Pantalla: un trapecio cálido.
        for j in 0..3 {
            for dx in -(2 + j)..=(2 + j) {
                canvas.solid(x as i32 + dx, y as i32 - 3 + j, warm.scale(0.35 + 0.6 * on));
            }
        }
        for dy in -6i32..=8 {
            for dx in -10i32..=10 {
                let d2 = (dx * dx) as f32 / 4.0 + (dy * dy) as f32;
                canvas.glow(x as i32 + dx, y as i32 + dy, warm, 0.10 * on * (-d2 / 18.0).exp());
            }
        }
    }

    /// Tocadiscos sobre el escritorio, visto desde arriba y de frente: base, plato, disco
    /// que gira y brazo. Con carátula, el disco la lleva impresa (un picture disc) y la
    /// funda queda apoyada atrás; sin ella, es un vinilo negro con etiqueta.
    fn draw_turntable(&self, canvas: &mut Canvas, desk: f32, pal: &Palette, warm: Rgb, on: f32) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let depth = h - desk;
        let a = (depth * 0.34 / 0.5).min(w * 0.12);
        if a < 3.0 {
            return;
        }
        let b = a * 0.5;
        let (cx, cy) = (w * TURNTABLE_X, desk + depth * 0.5);
        // La funda, acostada sobre el escritorio a la izquierda (vista desde arriba, como el
        // disco: el doble de ancha que de alta).
        if let Some(cover) = &self.cover {
            let width = (a * 1.7).floor();
            let height = (width * 0.5).floor();
            let right = cx - a * 1.35 - 3.0;
            if height >= 4.0 && right - width > 0.0 {
                let (x0, y0) = ((right - width) as i32, (cy - height * 0.4) as i32);
                let (nw, nh) = (width as i32, height as i32);
                for dy in -1..=nh {
                    for dx in -1..=nw {
                        let edge = dx < 0 || dy < 0 || dx >= nw || dy >= nh;
                        let c = if edge {
                            Rgb(0.07, 0.06, 0.06)
                        } else {
                            cover.at((dx as f32 + 0.5) / width, (dy as f32 + 0.5) / height).scale(0.5 + 0.4 * on).add(warm, 0.08 * on)
                        };
                        canvas.solid(x0 + dx, y0 + dy, c);
                    }
                }
                // Su canto, más oscuro, para que parezca apoyada y no pintada.
                for dx in 0..=nw {
                    canvas.solid(x0 + dx, y0 + nh + 1, Rgb(0.05, 0.04, 0.04));
                }
            }
        }
        // Base de madera oscura con su canto.
        let plinth = Rgb(0.09, 0.08, 0.08).add(warm, 0.10 * on);
        let (left, right) = ((cx - a * 1.35) as i32, (cx + a * 1.65) as i32);
        let (top, bottom) = ((cy - b * 1.4) as i32, (cy + b * 1.4) as i32);
        for y in top..=bottom + 1 {
            for x in left..=right {
                let c = if y > bottom { plinth.scale(0.6) } else { plinth };
                canvas.solid(x, y, c);
            }
        }
        // Plato, un poco más grande que el disco.
        let angle = self.record * TAU;
        let (sa, ca) = angle.sin_cos();
        let label = pal.petal;
        for y in (cy - b * 1.08) as i32..=(cy + b * 1.08) as i32 {
            for x in (cx - a * 1.08) as i32..=(cx + a * 1.08) as i32 {
                let (u, v) = ((x as f32 + 0.5 - cx) / a, (y as f32 + 0.5 - cy) / b);
                let r = (u * u + v * v).sqrt();
                if r > 1.08 {
                    continue;
                }
                if r > 1.0 {
                    canvas.solid(x, y, Rgb(0.30, 0.30, 0.32).scale(0.6 + 0.4 * on));
                    continue;
                }
                // Coordenadas sobre el disco que gira.
                let (ru, rv) = (u * ca - v * sa, u * sa + v * ca);
                let grooves = 0.92 + 0.08 * (r * 38.0).sin();
                let shine = ((u.atan2(v) * 2.0 + 0.6).sin() * 0.5 + 0.5).powi(6) * 0.25;
                let c = if r < 0.06 {
                    Rgb(0.75, 0.75, 0.78)
                } else if r > 0.94 {
                    Rgb(0.03, 0.03, 0.04)
                } else {
                    match &self.cover {
                        // La carátula entera dentro del disco: sus esquinas quedan fuera.
                        Some(cover) => cover.at(ru * 0.5 / 0.94 + 0.5, rv * 0.5 / 0.94 + 0.5).scale(0.5 + 0.35 * on),
                        None if r < 0.34 => label.scale(0.6 + 0.4 * on).mix(Rgb::WHITE, 0.15 * (ru * 6.0).sin().max(0.0)),
                        None => Rgb(0.04, 0.04, 0.05),
                    }
                };
                canvas.solid(x, y, c.scale(grooves).add(Rgb::WHITE, shine * (0.5 + 0.5 * on)));
            }
        }
        // Brazo: desde el pivote, atrás a la derecha, hasta la púa sobre el disco.
        let pivot = (cx + a * 1.35, cy - b * 0.95);
        let needle = (cx + a * 0.62, cy + b * 0.35);
        let steps = ((pivot.0 - needle.0).abs() + (pivot.1 - needle.1).abs()).ceil() as i32 * 2;
        let arm = Rgb(0.78, 0.78, 0.80).scale(0.55 + 0.45 * on);
        for k in 0..=steps {
            let t = k as f32 / steps.max(1) as f32;
            let (x, y) = (pivot.0 + (needle.0 - pivot.0) * t, pivot.1 + (needle.1 - pivot.1) * t);
            canvas.solid(x as i32, y as i32, arm);
        }
        canvas.solid(pivot.0 as i32, pivot.1 as i32, Rgb(0.5, 0.5, 0.52));
        canvas.solid(pivot.0 as i32 + 1, pivot.1 as i32, Rgb(0.5, 0.5, 0.52));
        // Luz de encendido que late con el pulso.
        canvas.glow(right - 1, bottom - 1, Rgb(1.0, 0.35, 0.25), 0.4 + 0.5 * self.beat_pulse);
    }

    fn draw_desk_things(&self, canvas: &mut Canvas, desk: f32, pal: &Palette, warm: Rgb, on: f32) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let base = desk as i32 - 1;
        self.draw_turntable(canvas, desk, pal, warm, on);
        // Taza con vapor que se apaga hacia el final de la canción.
        let (mx, my) = (w * 0.66, base);
        let mug = pal.accent.scale(0.45).add(warm, 0.2 * on);
        for dy in 0..3 {
            for dx in 0..3 {
                canvas.solid(mx as i32 + dx, my - dy, mug);
            }
        }
        canvas.solid(mx as i32 + 3, my - 1, mug);
        let steam = 1.0 - smoothstep(0.6, 1.0, self.progress);
        for k in 0..6 {
            let t = (self.time * 0.4 + k as f32 / 6.0).fract();
            let x = mx + 1.0 + (t * 6.0 + k as f32).sin() * 1.2;
            let y = my as f32 - 3.0 - t * h * 0.12;
            canvas.dot_over((x * 2.0) as i32, (y * 2.0) as i32, Rgb(0.9, 0.9, 0.9), 0.35 * steam * (1.0 - t));
        }
        // Notas que suben del tocadiscos.
        for n in &self.notes {
            let fade = 1.0 - n.age / 3.0;
            let (x, y) = ((n.x * w) as i32, (n.y * h) as i32);
            canvas.glyph(x, y / 2, '♪', pal.petal_light.mix(Rgb::WHITE, 0.3), fade.max(0.0));
        }
    }

    /// Gato sentado en el alféizar, de espaldas, mirando afuera.
    fn draw_cat(&self, canvas: &mut Canvas, cx: f32, sill: f32, warm: Rgb, on: f32) {
        let h = canvas.px_h() as f32;
        let s = (h * 0.018).clamp(0.8, 2.5);
        let fur = Rgb(0.05, 0.05, 0.06).add(warm, 0.12 * on);
        let base = sill as i32;
        // Cuerpo: una gota ancha abajo.
        let body_h = (7.0 * s) as i32;
        for j in 0..body_h {
            let v = j as f32 / body_h as f32;
            let half = (1.5 + 2.6 * v.powf(0.7)) * s;
            for dx in -(half as i32)..=half as i32 {
                canvas.solid(cx as i32 + dx, base - body_h + j, fur);
            }
        }
        // Cabeza y orejas: las orejas se paran con los platillos.
        let (hx, hy) = (cx as i32, base - body_h - (1.5 * s) as i32);
        let hr = (2.0 * s) as i32;
        for dy in -hr..=hr {
            for dx in -(hr + 1)..=(hr + 1) {
                if (dx * dx) as f32 / 1.4 + (dy * dy) as f32 <= (hr * hr) as f32 + 0.5 {
                    canvas.solid(hx + dx, hy + dy, fur);
                }
            }
        }
        let ear = (1.0 + 1.5 * self.hat_pulse) * s;
        for side in [-1, 1] {
            for k in 0..=ear as i32 {
                let x = hx + side * (hr - 1);
                canvas.solid(x, hy - hr - k, fur);
            }
        }
        // Cola: una curva que se mece de lado a lado con el pulso.
        let swing = (self.phase() * TAU).sin();
        let length = 6.0 * s;
        for i in 0..(length * 2.0) as i32 {
            let t = i as f32 / (length * 2.0);
            let x = cx + 3.5 * s + t * length * 0.6 + swing * t * t * length * 0.7;
            let y = base as f32 - 0.5 - t * length * 0.5 - (t * 3.0).sin() * s;
            canvas.solid(x as i32, y as i32, fur);
        }
    }

    fn draw_glass(&self, canvas: &mut Canvas) {
        let (w, h) = (canvas.w as f32, canvas.px_h() as f32);
        let (wl, wt, wr, wb) = (WINDOW.0 * w, WINDOW.1 * h, WINDOW.2 * w, WINDOW.3 * h);
        let drop_color = Rgb(0.80, 0.86, 0.95);
        for d in &self.drops {
            let (x, y) = (wl + d.x * (wr - wl), wt + d.y * (wb - wt));
            for k in 0..3 {
                canvas.dot_over((x * 2.0) as i32, (y * 2.0) as i32 - k, drop_color, 0.6 - 0.18 * k as f32);
            }
        }
    }
}

impl Scene for Lofi {
    fn update(&mut self, frame: &Frame, _w: usize, _h: usize) {
        self.advance(frame);
    }

    fn draw(&self, canvas: &mut Canvas, pal: &Palette) {
        let h = canvas.px_h() as f32;
        // Afuera: el cielo de la canción, con el horizonte a la altura del borde de abajo
        // de la ventana.
        let ground = (h * WINDOW.3).max(1.0);
        let view = View { cx: canvas.w as f32 / 2.0, ground, scale: ground.max(4.0), weight: 1.0 };
        let scenery = &self.scenery;
        let top = h * WINDOW.1;
        canvas.clear(|_, y| scenery.sky(((y as f32 - top) / (ground - top).max(1.0)).clamp(0.0, 1.0)).mix(pal.bg_bottom, 0.08));
        self.scenery.draw_back(canvas, &view, pal);
        self.scenery.draw_weather(canvas, &view);
        self.draw_glass(canvas);
        self.draw_room(canvas, pal);
    }

    fn new_track(&mut self) {
        if self.switching.is_none() {
            self.switching = Some(0.0);
        }
        self.scenery.fade();
    }

    fn poke(&mut self, x: f32, y: f32) {
        if self.notes.len() < MAX_NOTES {
            self.notes.push(Note { x, y, age: 0.0 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(bands: &Bands, progress: f32) -> Frame<'_> {
        Frame { dt: 1.0 / 60.0, bands, progress: Some(progress), song_len: 120.0, playing: true }
    }

    #[test]
    fn record_spins_only_while_playing_and_loud_beats_make_notes() {
        let mut room = Lofi::new(1);
        let loud = Bands { beat: true, intensity: 1.0, ..Bands::default() };
        for _ in 0..120 {
            room.update(&frame(&loud, 0.3), 120, 40);
        }
        assert!(room.record > 0.5);
        assert!(!room.notes.is_empty() && room.notes.len() <= MAX_NOTES);
        let spun = room.record;
        let paused = Frame { playing: false, ..frame(&loud, 0.3) };
        room.update(&paused, 120, 40);
        assert_eq!(room.record, spun);
    }

    #[test]
    fn rain_outside_runs_down_the_glass() {
        let mut room = Lofi::new(2);
        room.scenery.force_mood(Some(crate::mood::Mood::Sad));
        for _ in 0..600 {
            room.update(&frame(&Bands::default(), 0.3), 120, 40);
        }
        assert!(!room.drops.is_empty() && room.drops.len() <= MAX_DROPS);
        let mut dry = Lofi::new(3);
        dry.scenery.force_mood(Some(crate::mood::Mood::Happy));
        for _ in 0..600 {
            dry.update(&frame(&Bands::default(), 0.3), 120, 40);
        }
        assert!(dry.drops.len() < room.drops.len());
    }

    #[test]
    fn draws_at_any_size() {
        let mut room = Lofi::new(4);
        let bands = Bands { beat: true, hat: true, climax: true, treble: 0.8, intensity: 1.0, ..Bands::default() };
        for _ in 0..30 {
            room.update(&frame(&bands, 0.5), 80, 24);
        }
        room.poke(0.5, 0.5);
        for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24), (200, 60)] {
            let mut canvas = Canvas::new(w, h);
            room.draw(&mut canvas, &Palette::default());
        }
    }

    #[test]
    fn the_record_shows_the_cover() {
        let mut room = Lofi::new(5);
        let bands = Bands::default();
        room.update(&frame(&bands, 0.5), 120, 30);
        let draw = |room: &Lofi| {
            let mut canvas = Canvas::new(120, 30);
            room.draw(&mut canvas, &Palette::default());
            canvas.to_image()
        };
        let plain = draw(&room);
        // Una carátula verde pura: tiene que aparecer en el disco y en la funda.
        let mut png = Vec::new();
        image::RgbImage::from_pixel(8, 8, image::Rgb([0, 255, 0]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        room.cover = Cover::from_image_bytes(&png);
        let covered = draw(&room);
        let green = |img: &image::RgbImage| img.pixels().filter(|p| p[1] > 90 && p[0] < 40 && p[2] < 40).count();
        assert_eq!(green(&plain), 0);
        assert!(green(&covered) > 200, "{}", green(&covered));
    }
}

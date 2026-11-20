//! Lista de escenas para el modo automático.
//!
//! En automático, cada canción elige su escena. La lista dice entre cuáles elige y cómo
//! se turnan:
//!
//! - **según el ánimo** (por defecto): entre las de la lista que le van al ánimo de la
//!   canción. Si ninguna le va, cualquiera de la lista. Cada canción se queda siempre con
//!   la misma.
//! - **al azar**: cualquiera de la lista, sin repetir la anterior.
//! - **en orden**: una tras otra, en el orden de la tecla `e`.
//!
//! Se guarda en la configuración como `lista=jardin,ruta,bosque` (o `todas`) y
//! `orden=animo|azar|orden`.

use crate::mood::Mood;
use crate::scene::SceneKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Mood,
    Shuffle,
    Sequence,
}

impl Order {
    const ALL: [Order; 3] = [Order::Mood, Order::Shuffle, Order::Sequence];

    pub fn parse(name: &str) -> Option<Order> {
        match name.to_lowercase().as_str() {
            "animo" | "ánimo" | "mood" => Some(Order::Mood),
            "azar" | "aleatorio" | "shuffle" => Some(Order::Shuffle),
            "orden" | "seguido" | "order" => Some(Order::Sequence),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Order::Mood => "animo",
            Order::Shuffle => "azar",
            Order::Sequence => "orden",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Order::Mood => "según el ánimo",
            Order::Shuffle => "al azar",
            Order::Sequence => "en orden",
        }
    }

    pub fn step(self, step: i32) -> Order {
        let i = Self::ALL.iter().position(|o| *o == self).unwrap_or(0) as i32;
        Self::ALL[(i + step).rem_euclid(Self::ALL.len() as i32) as usize]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Playlist {
    /// Escenas elegidas, en el orden de [`SceneKind::ALL`]. Nunca queda vacía.
    scenes: Vec<SceneKind>,
    pub order: Order,
}

impl Default for Playlist {
    fn default() -> Self {
        Playlist { scenes: SceneKind::ALL.to_vec(), order: Order::Mood }
    }
}

/// Mezcla los bits de un número: para que canciones parecidas den escenas distintas.
fn mix(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^ (x >> 33)
}

impl Playlist {
    /// Una lista con estas escenas. Si viene vacía, todas: una lista vacía no sirve.
    pub fn new(scenes: &[SceneKind], order: Order) -> Playlist {
        let mut p = Playlist { scenes: Vec::new(), order };
        for kind in SceneKind::ALL {
            if scenes.contains(&kind) {
                p.scenes.push(kind);
            }
        }
        if p.scenes.is_empty() {
            p.scenes = SceneKind::ALL.to_vec();
        }
        p
    }

    /// Lee `jardin,ruta,bosque` o `todas`. Los nombres que no entiende los salta.
    pub fn parse(list: &str, order: &str) -> Playlist {
        let order = Order::parse(order.trim()).unwrap_or(Order::Mood);
        let wanted: Vec<SceneKind> = list.split(',').filter_map(|name| SceneKind::parse(name.trim())).collect();
        Playlist::new(&wanted, order)
    }

    /// Para la configuración: `todas` o los nombres separados por coma.
    pub fn key(&self) -> String {
        if self.is_all() {
            "todas".to_string()
        } else {
            self.scenes.iter().map(|k| k.key()).collect::<Vec<_>>().join(",")
        }
    }

    pub fn scenes(&self) -> &[SceneKind] {
        &self.scenes
    }

    pub fn len(&self) -> usize {
        self.scenes.len()
    }

    pub fn is_all(&self) -> bool {
        self.scenes.len() == SceneKind::ALL.len()
    }

    pub fn contains(&self, kind: SceneKind) -> bool {
        self.scenes.contains(&kind)
    }

    /// Agrega o saca una escena. Devuelve si quedó adentro. La última no se puede sacar.
    pub fn toggle(&mut self, kind: SceneKind) -> bool {
        if let Some(i) = self.scenes.iter().position(|k| *k == kind) {
            if self.scenes.len() > 1 {
                self.scenes.remove(i);
                return false;
            }
            return true;
        }
        let mut wanted = self.scenes.clone();
        wanted.push(kind);
        *self = Playlist::new(&wanted, self.order);
        true
    }

    /// Todas adentro; si ya estaban todas, deja solo `keep` (para empezar a armar desde cero).
    pub fn toggle_all(&mut self, keep: SceneKind) {
        *self = if self.is_all() { Playlist::new(&[keep], self.order) } else { Playlist::new(&SceneKind::ALL, self.order) };
    }

    /// Resumen para el menú: "todas · según el ánimo", "5 escenas · en orden".
    pub fn summary(&self) -> String {
        let what = match self.scenes.len() {
            n if n == SceneKind::ALL.len() => "todas".to_string(),
            1 => format!("solo {}", self.scenes[0].name()),
            n => format!("{n} escenas"),
        };
        format!("{what} · {}", self.order.name())
    }

    /// ¿Hay que esperar a conocer el ánimo para elegir? Solo en el orden según el ánimo.
    pub fn needs_mood(&self) -> bool {
        self.order == Order::Mood && self.scenes.len() > 1
    }

    /// La escena para una canción. `salt`: fijo por canción; `previous`: la escena que se
    /// estaba mostrando en automático (`None` en la primera); `turn`: cuántas canciones van.
    pub fn pick(&self, mood: Mood, salt: u64, previous: Option<SceneKind>, turn: u32) -> SceneKind {
        let list = &self.scenes;
        let n = list.len();
        match self.order {
            Order::Mood => {
                let fits: Vec<SceneKind> = SceneKind::for_mood_all(mood).iter().copied().filter(|k| list.contains(k)).collect();
                let options = if fits.is_empty() { list.as_slice() } else { fits.as_slice() };
                options[(salt % options.len() as u64) as usize]
            }
            Order::Shuffle => {
                let r = mix(salt ^ (turn as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
                let mut i = (r % n as u64) as usize;
                if n > 1 && Some(list[i]) == previous {
                    // Otra cualquiera, pero no la misma.
                    i = (i + 1 + ((r >> 32) % (n as u64 - 1)) as usize) % n;
                }
                list[i]
            }
            Order::Sequence => {
                let Some(prev) = previous else { return list[0] };
                if let Some(i) = list.iter().position(|k| *k == prev) {
                    return list[(i + 1) % n];
                }
                // La anterior ya no está en la lista: la que le seguía en el orden general.
                let at = SceneKind::ALL.iter().position(|k| *k == prev).unwrap_or(0);
                (1..=SceneKind::ALL.len())
                    .map(|d| SceneKind::ALL[(at + d) % SceneKind::ALL.len()])
                    .find(|k| list.contains(k))
                    .unwrap_or(list[0])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use SceneKind::*;

    #[test]
    fn parses_and_writes_back() {
        let p = Playlist::parse("ruta, bosque,nada,jardin", "orden");
        // En el orden general, sin los nombres que no existen.
        assert_eq!(p.scenes(), [Garden, Coast, Forest]);
        assert_eq!(p.order, Order::Sequence);
        assert_eq!(Playlist::parse(&p.key(), p.order.key()), p);
        assert_eq!(Playlist::parse("todas", "animo"), Playlist::default());
        assert_eq!(Playlist::default().key(), "todas");
        // Vacía o basura: todas.
        assert!(Playlist::parse("", "???").is_all());
        assert_eq!(Playlist::parse("", "???").order, Order::Mood);
    }

    #[test]
    fn toggling_never_leaves_it_empty() {
        let mut p = Playlist::new(&[Coast], Order::Mood);
        assert!(p.toggle(Coast), "la última se queda");
        assert!(p.toggle(Storm));
        assert_eq!(p.scenes(), [Coast, Storm]);
        assert!(!p.toggle(Coast));
        assert_eq!(p.scenes(), [Storm]);
        p.toggle_all(Storm);
        assert!(p.is_all());
        p.toggle_all(Pond);
        assert_eq!(p.scenes(), [Pond]);
    }

    #[test]
    fn by_mood_stays_inside_the_list() {
        let p = Playlist::new(&[Garden, City, Alley], Order::Mood);
        for salt in 0..50 {
            assert_eq!(p.pick(Mood::Happy, salt, None, 0), Garden);
            assert_eq!(p.pick(Mood::Intense, salt, None, 0), City);
            let sad = p.pick(Mood::Sad, salt, None, 0);
            assert_eq!(sad, Alley);
            // A la serena no le va ninguna de la lista: sale cualquiera de la lista.
            assert!(p.contains(p.pick(Mood::Serene, salt, None, 0)));
        }
        // Con todas, es lo de siempre.
        let all = Playlist::default();
        for salt in 0..20 {
            assert_eq!(all.pick(Mood::Serene, salt, None, 0), SceneKind::for_song(Mood::Serene, salt));
        }
    }

    #[test]
    fn shuffle_never_repeats_and_reaches_everything() {
        let p = Playlist::new(&[Garden, Ocean, Storm, Pond], Order::Shuffle);
        let mut prev = None;
        let mut seen = std::collections::HashSet::new();
        for turn in 0..200 {
            let k = p.pick(Mood::Happy, 12345, prev, turn);
            assert!(p.contains(k));
            assert_ne!(Some(k), prev);
            seen.insert(k);
            prev = Some(k);
        }
        assert_eq!(seen.len(), 4);
    }

    #[test]
    fn in_order_goes_one_after_another() {
        let p = Playlist::new(&[Ocean, Forest, Village], Order::Sequence);
        assert_eq!(p.pick(Mood::Happy, 0, None, 0), Ocean);
        assert_eq!(p.pick(Mood::Happy, 0, Some(Ocean), 1), Forest);
        assert_eq!(p.pick(Mood::Happy, 0, Some(Forest), 2), Village);
        assert_eq!(p.pick(Mood::Happy, 0, Some(Village), 3), Ocean);
        // Si la anterior se sacó de la lista, sigue con la que venía después.
        assert_eq!(p.pick(Mood::Happy, 0, Some(Coast), 4), Forest);
        assert_eq!(p.pick(Mood::Happy, 0, Some(Balloons), 4), Ocean);
    }

    #[test]
    fn summary_reads_well() {
        assert_eq!(Playlist::default().summary(), "todas · según el ánimo");
        assert_eq!(Playlist::new(&[Coast], Order::Shuffle).summary(), "solo ruta costera · al azar");
        assert_eq!(Playlist::new(&[Coast, Pond], Order::Sequence).summary(), "2 escenas · en orden");
    }
}

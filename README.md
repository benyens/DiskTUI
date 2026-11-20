<div align="center">

# 🌸 DiskTUI

**Tu música, convertida en paisaje dentro de la terminal.**

Un visualizador de audio para macOS y Windows, escrito en Rust. Escucha la canción, entiende su
ánimo y la convierte en una escena viva: un jardín que florece, fuegos artificiales sobre
un lago, un tren de noche, medusas en el fondo del mar. Todo con los colores de la
carátula.

![Rust](https://img.shields.io/badge/Rust-1.89%2B-orange?logo=rust)
![macOS](https://img.shields.io/badge/macOS-Apple%20Silicon%20%7C%20Intel-lightgrey?logo=apple)
![Windows](https://img.shields.io/badge/Windows-experimental-0078D6?logo=windows)
![TUI](https://img.shields.io/badge/TUI-ratatui-blue)
![audio local](https://img.shields.io/badge/audio-100%25%20local-green)
![Vibecoded](https://img.shields.io/badge/vibecoded-con%20Claude-8A2BE2)

<img src="docs/escenas.gif" alt="Las dieciocho escenas de DiskTUI, una tras otra" width="90%">

<sub>Las dieciocho escenas, generadas desde la salida real del programa (`--demo`).</sub>

**[Instalar](#instalación) · [Usar](#uso) · [Escenas](#escenas) · [Cómo entiende la canción](#cómo-entiende-la-canción) · [Vibecoded](#hecho-con-vibe-coding)**

</div>

---

## Qué hace

- 🎭 **Entiende el ánimo de la canción** (alegre, serena, nostálgica, triste, intensa) y
  elige escena y clima: sol, atardecer, noche, lluvia o tormenta. El clima cambia dentro de
  la canción: el verso calma, el coro agita.
- 🥁 **Sigue el pulso**: estima el tempo y predice cada golpe; la escena se mece al compás.
- 💥 **Siente los clímax**: sabe cuándo entra el coro o cae el drop, y la escena estalla.
- 🌼 **Una flor única por canción**, guardada en tu **herbario**, con póster para compartir.
  Marca tus favoritas con `f` y filtra el herbario por ellas.
- 🖱️ **Click en la escena** y responde: un fuego artificial, una mariposa, un pez, un cometa…
- 🎬 **Tarjeta con el título** al empezar cada canción (se apaga con `t`).
- 🦋 **Transiciones entre canciones**: mariposas, pétalos, iris, ola o destellos.
- 🎨 **Colores de la carátula** en todo: pétalos, velas, fuegos, medusas.
- 🎧 **Spotify, Apple Music o archivos** (mp3, flac, wav, ogg, m4a).
- 📸 **Para compartir**: postal PNG, GIF, y GIF vertical con título para historias. Con
  las letras encendidas, la línea que suena va de subtítulo.
- 🎤 **Letras sincronizadas** (opcional) que se iluminan mientras se cantan, y un **modo
  karaoke** (`k`) con la línea en grande.
- 🔒 **Local y sin cuentas.** El audio se analiza en tu equipo. Las únicas consultas a
  internet son la carátula en modo Spotify y, si las enciendes, las letras.

## Escenas

| | |
| :---: | :---: |
| <img src="docs/jardin.png" alt="Jardín"> | <img src="docs/fuegos.png" alt="Fuegos artificiales"> |
| 🌱 **Jardín** · un brote sale de la tierra y florece | 🎆 **Fuegos artificiales** · un festival de verano junto al lago |
| <img src="docs/oceano.png" alt="Océano"> | <img src="docs/tren.png" alt="Tren nocturno"> |
| ⛵ **Océano** · un velero cruza el mar hasta el faro | 🚆 **Tren nocturno** · un poste cruza la ventana en cada pulso |
| <img src="docs/fondo.png" alt="Fondo del mar"> | <img src="docs/fogata.png" alt="Fogata"> |
| 🪼 **Fondo del mar** · medusas que laten con el bombo | 🔥 **Fogata** · arde al ritmo y queda en brasas al final |
| <img src="docs/sistema.png" alt="Sistema solar"> | <img src="docs/ciudad.png" alt="Ciudad"> |
| 🪐 **Sistema solar** · estrellas, un sol y planetas al azar | 🌆 **Ciudad** · edificios que suben y ventanas que siguen la música |
| <img src="docs/cerezos.png" alt="Cerezos"> | <img src="docs/lofi.png" alt="Cuarto lofi"> |
| 🌸 **Cerezos** · un sakura florece y suelta pétalos con cada golpe | 🐈 **Cuarto lofi** · lluvia en la ventana, un gato al compás y un tocadiscos con la carátula de lo que suena |
| <img src="docs/aurora.png" alt="Aurora boreal"> | <img src="docs/mariposas.png" alt="Mariposas"> |
| 🌌 **Aurora boreal** · cortinas de luz sobre un lago helado | 🦋 **Mariposas** · una migración que cruza la pradera en diagonal hacia arriba |
| <img src="docs/ruta.png" alt="Ruta costera"> | <img src="docs/bosque.png" alt="Bosque"> |
| 🚗 **Ruta costera** · un auto maneja junto al mar; el tempo pone la velocidad | 🌳 **Bosque** · brota, crece y florece durante la canción |
| <img src="docs/concierto.png" alt="Concierto"> | <img src="docs/callejon.png" alt="Callejón"> |
| 🎤 **Concierto** · pantalla LED, luces que barren al ritmo y un público que salta | 🏮 **Callejón** · neón, faroles y lluvia en un callejón japonés que se refleja en el suelo mojado |
| <img src="docs/estanque.png" alt="Estanque"> | <img src="docs/tormenta.png" alt="Tormenta en alta mar"> |
| 🐟 **Estanque** · kois que nadan entre nenúfares y flores de loto | ⛈️ **Tormenta** · un barco entre olas que crecen con los graves; relámpagos en los clímax |

Con la escena en *automática* (el valor por defecto), cada canción elige la suya según su
ánimo. La tecla `e` las recorre a mano.

<div align="center">
<img src="docs/climas.png" alt="El mismo jardín bajo cuatro ánimos: alegre, serena, triste e intensa" width="90%">
<br>
<sub>El mismo jardín con cuatro ánimos: alegre, serena, triste e intensa.</sub>
</div>

## Instalación

Necesitas macOS, [Rust](https://rustup.rs) 1.89 o superior y una terminal con color de
24 bits (Ghostty, iTerm2, Kitty, WezTerm). Sin eso último cae solo a 256 colores.

Con Homebrew:

```sh
brew tap benyens/disktui https://github.com/benyens/DiskTUI
brew install --HEAD disktui
```

O desde el código:

```sh
git clone https://github.com/benyens/DiskTUI && cd DiskTUI
cargo install --path .
```

La primera compilación tarda un par de minutos. Queda el comando `disktui`:

```sh
disktui
```

Si la terminal no lo encuentra, agrega `~/.cargo/bin` a tu PATH:
`echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc` y abre otra terminal.

Para probar sin instalar: `cargo run --release`.

### En Windows (experimental)

Funcionan el demo, el modo archivo y el **modo Spotify**. En Windows, DiskTUI escucha todo
el audio del sistema (no solo el de Spotify) y lee título, artista, carátula y posición de
los controles multimedia de Windows, los mismos del panel de volumen. Play/pausa, `n`/`p` y
las flechas funcionan; el volumen se maneja con el de Windows.

1. Instala las herramientas de compilación de C++ y Rust, en PowerShell:

   ```powershell
   winget install Microsoft.VisualStudio.2022.BuildTools --override "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --passive --wait"
   winget install Rustlang.Rustup
   ```

   Cierra y abre la terminal para que encuentre `cargo`.

2. Instala DiskTUI:

   ```powershell
   cargo install --git https://github.com/benyens/DiskTUI
   ```

3. Ábrelo en **Windows Terminal** (tiene color de 24 bits):

   ```powershell
   disktui                  # menú, con Spotify por defecto
   disktui --spotify
   disktui --demo
   disktui "C:\ruta\a\cancion.mp3"
   ```

En Windows, la configuración, el herbario y las correcciones quedan en
`%APPDATA%\DiskTUI`, y las imágenes en `Imágenes\DiskTUI`. Si se ve entrecortado, prueba
`--fps 30`.

## Uso

`disktui` a secas abre el **menú de inicio**, sobre una vista previa animada:

<div align="center"><img src="docs/menu.png" alt="Menú de inicio" width="70%"></div>

- Viene apuntando a **Spotify**: Enter y empieza.
- `↑` `↓` eligen fila, `←` `→` cambian el valor, `Esc` sale.
- Con fuente *Archivo*, escribe la ruta o arrastra el archivo a la terminal.
- Lo elegido se guarda en `~/.config/disktui/config` y vuelve la próxima vez.
- Durante la música, `m` vuelve al menú.
- `g` abre el herbario.

También se puede saltar el menú desde la línea de comandos:

| Comando | Qué hace |
| --- | --- |
| `disktui <archivo>` | Reproduce el archivo y lo visualiza |
| `disktui --spotify` | Visualiza lo que suena en la app de Spotify |
| `disktui --apple-music` | Lo mismo con Apple Music (experimental) |
| `disktui --herbario` | Muestra las flores de las canciones que ya escuchaste |
| `disktui --demo` | Animación con señal sintética, sin audio |
| `disktui --diagnostico` | Prueba el modo Spotify paso a paso y dice qué falla |

| Opción | Efecto |
| --- | --- |
| `--escena <nombre>` | `auto` (por defecto: la elige el ánimo), `jardin`, `sistema`, `oceano`, `ciudad`, `fogata`, `tren`, `fondo`, `fuegos`, `cerezos`, `lofi`, `aurora`, `mariposas`, `ruta`, `bosque`, `concierto`, `callejon`, `estanque` o `tormenta` |
| `--flores <n>` | Flores del jardín, de 1 a 3. Por defecto 3 |
| `--petalos <n>` | Pétalos de la flor principal, de 3 a 12 |
| `--velocidad <x>` | Qué tan rápido crece la escena. `2` = el doble. Por defecto 1 |
| `--mini` | Para un panel chico (tmux): HUD de una línea, una flor, 30 FPS |
| `--caratula` | Muestra la carátula del disco en una esquina |
| `--animo <nombre>` | Fija el clima: `alegre`, `serena`, `nostalgica`, `triste`, `intensa`. Por defecto lo decide la canción |
| `--fps <n>` | Cuadros por segundo. Por defecto 60 |
| `--sin-hud` | Arranca sin el HUD |
| `--sin-tarjeta` | No muestra la tarjeta con el título al empezar cada canción |
| `--letras` | Letras sincronizadas desde LRCLIB |
| `--ahorro` | Gasta menos batería: 30 FPS como máximo |
| `--dormir <minutos>` | Temporizador para dormir: baja el volumen al final y pausa |
| `--hora-real` | El cielo sigue la hora de tu reloj |
| `--hora <HH:MM>` | Fija la hora del cielo, para probar |
| `--transicion <nombre>` | Entre canciones: `auto` (una distinta cada vez), `fundido`, `mariposas`, `petalos`, `iris`, `ola` o `destellos` |
| `--dispositivo <nombre>` | Modo Spotify: captura desde esa entrada de audio |
| `--global` | Modo Spotify: captura todo el audio del sistema |

| Tecla | Acción |
| --- | --- |
| `e` | Cambiar de escena: automática, jardín, sistema solar, océano, ciudad, fogata, tren, fondo del mar, fuegos artificiales, cerezos, cuarto lofi, aurora boreal, mariposas, ruta costera, bosque, concierto, callejón, estanque, tormenta |
| `←` / `→` | Retroceder / adelantar 10 segundos (archivo, Spotify, Apple Music) |
| `g` | Grabar un GIF de 8 segundos en `~/Pictures/DiskTUI` (`g` otra vez lo corta antes) |
| `v` | Grabar un GIF vertical (9:16) con carátula y título, para historias |
| `c` | Mostrar u ocultar la carátula |
| `t` | Encender o apagar la tarjeta con el título de cada canción |
| `l` | Encender o apagar las letras sincronizadas |
| `k` | Modo karaoke: la línea que suena en grande, al centro |
| `f` | Marcar o desmarcar la canción como favorita |
| `a` | Aleatorio en Spotify o Apple Music (encender o apagar) |
| `r` | Repetir en Spotify (sí o no) o Apple Music (lista, canción, nada) |
| `z` | Temporizador para dormir: 15, 30, 45, 60, 90 minutos o apagado |
| click | La escena responde donde hiciste click |
| `d` | Panel de detalles: qué entendió de la canción, la fuente de audio y los avisos completos. Con el panel abierto, `1`–`5` corrigen el ánimo y `0` lo devuelve a automático |
| `n` / `p` | Canción siguiente / anterior (Spotify y Apple Music) |
| `s` | Guardar una postal PNG en `~/Pictures/DiskTUI` |
| `m` | Volver al menú |
| `espacio` | Pausa |
| `+` / `-` | Volumen |
| `h` | Mostrar u ocultar el HUD |
| `q`, `Esc`, `Ctrl-C` | Salir |

## HUD

```
 ♪ Título — Artista              96 BPM · La menor · nostálgica  │  jardín · vol 80%
              ▂▄▆███████▇▅▃                 ▃▅▇██████████
 ▶ 1:23 ▁▁▁▂▃▅██████████████▇▅▃▂▁▁▁▁▁▁▁▂▃▅▇██████████████▇▅▂▁ 3:45  G▆ M▄ A▂
```

- **Barra de reproducción.** Dibuja la intensidad de toda la canción como una onda. Lo ya
  escuchado va en los colores de la carátula, la posición actual en blanco y lo que falta
  en gris. Los clímax se marcan: más claros si ya pasaron, en color de acento si vienen.
- **Modo archivo:** la onda completa aparece al abrir, porque la canción se pre-analiza.
- **Spotify y demo:** la onda se va dibujando a medida que suena.
- **`G` `M` `A`:** medidores de graves, medios y agudos.
- **Avisos.** Si algo falla (sin señal, volumen 0, sin datos de la canción), arriba a la
  derecha aparece un aviso corto. El detalle completo y la fuente de audio están en el panel
  de la tecla `d`.
- En terminales de menos de 16 filas queda una sola línea. `h` lo oculta entero.

## Cómo entiende la canción

Todo es procesamiento de señal clásico, sin modelos entrenados y en tiempo real. Ver
[`src/features.rs`](src/features.rs) y [`src/mood.rs`](src/mood.rs).

| Qué | Cómo | Para qué |
| --- | --- | --- |
| **Pulso** | Flujo espectral 100 veces por segundo, autocorrelación para el tempo y un peine para la fase. Los graves pesan el triple al fijar la fase, para caer en el bombo y no en el contratiempo | Predice cada golpe: los tallos se mecen al compás y el sol late con él |
| **Tonalidad** | Croma de 12 notas desde una FFT de 8192, comparado con los perfiles de Krumhansl-Kessler de las 24 tonalidades | Mayor o menor |
| **Brillo** | Centroide espectral | Timbre opaco o brillante |
| **Densidad** | Ataques por segundo | Cuánto pasa en la canción |
| **Estéreo** | Energía de cada canal | La flor izquierda oye más el canal izquierdo; la derecha, el derecho |

Con eso se ubica la canción en un plano de dos ejes: **valencia** (triste ↔ alegre; pesan el
modo y el brillo) y **activación** (calmada ↔ enérgica; pesan el tempo y la densidad). Cinco
climas marcan puntos de ese plano y la escena se mezcla entre ellos de forma continua:

| Ánimo | Canción | Clima | Crece |
| --- | --- | --- | --- |
| alegre | mayor, rápida, brillante | día soleado, nubes blancas, mariposas | más rápido |
| serena | mayor, lenta | atardecer naranja, sol bajo | normal |
| nostálgica | intermedia | noche con luna, estrellas y aurora, luciérnagas | normal |
| triste | menor, lenta, opaca | cielo gris, lluvia | más lento |
| intensa | menor, rápida | tormenta, viento, relámpagos en los clímax | más rápido |

En modo archivo el ánimo se conoce al abrir, porque la canción se pre-analiza entera. En
Spotify se va afinando durante los primeros 20 segundos y el clima cambia de a poco. El HUD
muestra lo entendido: `96 BPM · La menor · nostálgica`.

Es una heurística: acierta el carácter general de una canción, no sus matices. Si no te
convence para un tema, `--animo` o la fila *Clima* del menú lo fijan a mano.

### Dentro de la canción

El ánimo es de la canción entera, pero el clima se mueve con cada parte. DiskTUI sigue la
energía de la sección que suena (la intensidad suavizada en unos 5 segundos; un clímax
entra de golpe) y lleva el clima hacia la versión más agitada o más calma del mismo ánimo:

| Canción | Parte suave | Parte fuerte |
| --- | --- | --- |
| de valencia baja (triste, intensa) | lluvia calma | tormenta con viento |
| de valencia alta (alegre, serena) | atardecer | sol pleno |

<div align="center"><img src="docs/secciones.png" alt="Una canción triste y una alegre en su parte suave y en su parte fuerte" width="90%"></div>

El panel de la tecla `d` muestra los dos: `triste (auto) · ahora intensa`.

### Momento del drop

En modo archivo, que conoce los clímax de antemano, la escena se frena y se apaga el último
medio segundo antes de cada uno y lo suelta con un destello. En Spotify no hay aviso previo,
así que queda solo el destello.

### Escena automática

Con la escena en *automática*, cada canción elige también dónde ocurre. Si un ánimo tiene
varias escenas, cada canción se queda siempre con la misma:

| Ánimo | Escena |
| --- | --- |
| alegre | 🌼 jardín al sol, 🎆 fuegos artificiales, 🌸 cerezos, 🦋 mariposas, 🚗 ruta costera, 🌳 bosque, 🐟 estanque o 🎤 concierto |
| serena | 🔥 fogata al atardecer, 🪼 fondo del mar, 🌸 cerezos, 🌌 aurora boreal, 🦋 mariposas, 🌳 bosque o 🐟 estanque |
| nostálgica | 🪐 sistema solar, 🚆 tren nocturno, 🎆 fuegos artificiales, 🐈 cuarto lofi, 🌌 aurora boreal o 🚗 ruta costera de noche o 🏮 callejón |
| triste | ⛵ océano con lluvia, 🚆 tren bajo la lluvia, 🐈 cuarto lofi o 🏮 callejón |
| intensa | 🌆 ciudad con tormenta, 🚗 ruta costera a toda velocidad, 🎤 concierto o ⛈️ tormenta en alta mar |

La elección se hace una vez por canción, cuando el análisis ya es fiable: de inmediato en
modo archivo, a los ~20 segundos en Spotify. Las escenas se funden una en otra. La tecla
`e` recorre automática y las dieciocho escenas.

### Click en la escena

Con el mouse, un click hace que la escena responda donde lo hiciste:

| Escena | Click |
| --- | --- |
| jardín | de día llega una mariposa; de noche, una luciérnaga que destella |
| fuegos artificiales, ciudad | un fuego artificial estalla ahí |
| cerezos | una nube de pétalos |
| océano | salta un pez |
| sistema solar, aurora boreal | un cometa o una estrella fugaz |
| fondo del mar | burbujas |
| fogata | la avivas: una lluvia de chispas |
| tren nocturno | pasa un pueblo iluminado |
| cuarto lofi | una nota musical flota |
| mariposas | una bandada despega desde ahí |
| ruta costera | cruza una bandada de gaviotas |
| bosque | todo florece un momento y salen pétalos |
| concierto | una lluvia de confeti |
| callejón | una onda en el charco |
| estanque | una onda, y los peces van hacia ahí como si les tiraran comida |
| tormenta | cae un rayo ahí |

### Tarjeta de la canción

Al empezar cada canción aparece unos segundos su título y artista en grande, y se
desvanece. `t` la apaga o enciende, `--sin-tarjeta` arranca sin ella, y en el menú queda
guardada en la fila *Tarjeta*.

### Batería

Dibujar la escena cuesta poco (1–2 ms por cuadro); lo que gasta es que la terminal redibuje
toda la pantalla 60 veces por segundo. Por eso, en pausa o en la pantalla de espera baja a
15 FPS, y la fila *Energía* del menú decide el resto:

| Energía | Qué hace |
| --- | --- |
| ahorra con batería (por defecto) | 60 FPS enchufado, 30 con batería (macOS y Linux) |
| siempre fluida | 60 FPS siempre, salvo en pausa |
| ahorro | 30 FPS siempre (`--ahorro`) |

Además, los cambios de color tan chicos que no se notan (el cielo que se aclara de a poco,
un brillo que late apenas) no se mandan a la terminal: en promedio viaja menos de la mitad
de los datos por cuadro. El panel de detalles (`d`) muestra la fila *rendimiento*: los FPS
de ahora, cuánto de la pantalla cambia por cuadro y si anda con batería. Las mediciones están en los tests ignorados `bench_scenes`,
`bench_terminal` y `bench_analysis` (`cargo test --release bench -- --ignored --nocapture`).

### Temporizador para dormir

`z` recorre 15, 30, 45, 60 y 90 minutos (y apagado), o `--dormir 30` al abrir. El HUD
muestra ☾ con los minutos que faltan. En los últimos minutos baja de a poco el volumen de
Spotify o Apple Music y la escena se va haciendo de noche; al final pausa y deja el volumen
como estaba. En Windows solo pausa al final: el volumen de la app no se puede fijar desde
afuera.

### Letras sincronizadas

Con la tecla `l`, la fila *Letras* del menú o `--letras`, la letra de lo que suena aparece
abajo (arriba en el cuarto lofi, para no tapar el tocadiscos), línea por línea, siguiendo la posición de la canción en Spotify. La línea actual se
ilumina mientras se canta y la siguiente espera debajo.

Las letras vienen de [LRCLIB](https://lrclib.net), una base de letras libre que no pide
cuenta. Vienen apagadas porque es una consulta a internet: solo se mandan artista, título y
duración. Cada letra queda guardada en `letras/`, junto al herbario, y no se vuelve a pedir.
Si una canción no tiene letra, o la quieres corregir, deja ahí un `.lrc` propio con el
nombre `Artista - Título.lrc`.

Con `k` entra el **modo karaoke**: la línea que suena aparece en grande al centro de la
escena, con letras de píxeles que se van llenando de color mientras se canta, y la
siguiente espera debajo en chico. Si las letras estaban apagadas, `k` las enciende. La
letra nunca mueve la escena: solo se dibuja encima.

### Cielo según tu reloj

Con *Cielo: la hora de tu reloj* en el menú (o `--hora-real`), el cielo sigue la hora
real: de noche hay luna y estrellas aunque la canción sea alegre, y al atardecer el cielo
se pone naranja. El ánimo sigue decidiendo el clima: nubes, lluvia, viento y tormenta.
Los fuegos artificiales, la aurora y el cuarto lofi son siempre de noche.

`--hora 19:30` fija una hora para ver cómo queda.

### Transiciones

Entre canción y canción, la escena nueva entra con una transición. En *automática* (el
valor por defecto) cada canción trae una distinta y nunca repite la anterior:

<div align="center"><img src="docs/transiciones.gif" alt="Las seis transiciones" width="80%"></div>

| Transición | Cómo entra la escena nueva |
| --- | --- |
| mariposas | una bandada cruza en diagonal hacia arriba y la va descubriendo |
| pétalos | cae una cortina de pétalos de arriba abajo |
| iris | un círculo que se abre, con un anillo de luz en el borde |
| ola | una ola barre la pantalla de izquierda a derecha |
| destellos | se disuelve en cuadritos que brillan al cambiar |
| fundido | se funde con la anterior, sin adornos |

Se elige en el menú (fila *Transición*) o con `--transicion <nombre>`. Cuando la escena
automática cambia de escena dentro de una canción, usa la misma transición.

Con Spotify, la transición empieza justo cuando termina la canción, sin esperar a que la
app avise de la siguiente.

### Cuando se equivoca

La tecla `d` abre un panel con lo que DiskTUI midió: valencia, activación, tempo, tonalidad
y la confianza de cada uno, más la fuente de audio y los avisos completos. Con el panel
abierto, `1` a `5` fijan el ánimo de **esa canción** y `0` lo devuelve a automático. La
corrección se recuerda para la próxima vez que suene.

Las correcciones quedan en `~/.local/share/disktui/animos.tsv`, junto a lo que el análisis
había medido. Ese archivo es la base para calibrar la heurística con música real.

## Flor única y herbario

Cada canción da su propia flor. El **tempo** fija el número de pétalos (de 5 a 9), el
**modo** su forma (redondos en mayor, puntiagudos en menor), la **energía** el tamaño y la
cantidad de capas, y la **tónica** corre un poco el tono del color de la carátula. La misma
canción da siempre la misma flor.

Cuando escuchas al menos el 60% de una canción, su flor queda guardada. `disktui --herbario`
(o `g` en el menú) muestra la colección:

<div align="center"><img src="docs/herbario.png" alt="Herbario" width="80%"></div>

Arriba, un resumen de la semana: cuántas flores, el ánimo más escuchado y el BPM promedio.

**Favoritas.** Mientras suena una canción, `f` la marca como favorita (la nota del HUD se
vuelve ★) y otra vez la desmarca. En el herbario las favoritas llevan ★ y la tecla `f`
muestra solo esas. Quedan en `favoritas.tsv`, junto al herbario.

**Resumen de lo que escuchas.** En el herbario, `r` abre tu resumen de la semana, el mes o
desde siempre (←→ para cambiar): cuántas canciones, los artistas que más sonaron, cómo se
repartió el ánimo, el tempo promedio, la tonalidad más escuchada, la canción más repetida y
el día con más música. `p` lo guarda como imagen para compartir:

<div align="center"><img src="docs/resumen.png" alt="Resumen del mes" width="60%"></div>

La tecla `p` guarda un **póster** PNG con todas las flores, su título, artista y fecha:

<div align="center"><img src="docs/poster.png" alt="Póster del herbario" width="60%"></div>

## Compartir

Todo queda en `~/Pictures/DiskTUI`:

| Tecla | Qué guarda |
| --- | --- |
| `s` | La escena del momento como PNG |
| `g` | 8 segundos de la escena como GIF |
| `v` | 8 segundos en vertical (9:16) con carátula, título y artista abajo, para historias |
| `p` en el herbario | Póster con todas las flores |

Si la canción termina o sales a mitad de una grabación, el GIF se guarda igual con lo
grabado hasta ahí.

Con las letras encendidas, la postal y el GIF llevan de subtítulo la línea que sonaba,
entre comillas, en una franja oscura abajo. En el GIF el subtítulo cambia con la canción.

## Qué mueve cada cosa

Cada escena traduce las mismas señales a su manera.

<details>
<summary><b>🌱 Jardín y 🪐 Sistema solar</b></summary>

| Señal | 🌱 Jardín | 🪐 Sistema solar |
| --- | --- | --- |
| Avance de la canción | Altura del tallo; llega arriba pasada la mitad | Estrellas (0–20%), sol (5–20%), planetas (20–68%) |
| Graves, 20–250 Hz | Aceleran el crecimiento, mecen el tallo, ondas en la tierra | El sol late |
| Bombo | Brota una hoja, sube un pulso de luz | Una onda sale del sol e ilumina las órbitas |
| Medios, 250–4000 Hz | Abren la flor y la hacen girar | Los planetas giran más rápido |
| Agudos, 4–20 kHz | Tamaño de la flor, polen | Las estrellas titilan |
| Platillo | Chispas, destella una luciérnaga | Nace una estrella, otra destella, a veces cruza un cometa |
| Volumen | Resplandor de la flor, halo de la luna | Corona del sol |
| Intensidad | Aurora en el cielo, brillo del fondo, polen, luciérnagas | Nebulosas, largo de las estelas, velocidad |
| **Clímax** | La flor se abre de golpe y suelta pétalos y chispas | Onda de choque y lluvia de cometas |
| Carátula | Pétalos, pistilo y tallo | Sol, planetas y nebulosas |
| Pulso | Vaivén de los tallos, latido de las flores | Latido del sol |
| Ánimo | Cielo, clima y velocidad de crecimiento | Espacio frío o cálido, velocidad de las órbitas; lluvia de estrellas si es triste, erupciones del sol si es intensa |
| Cambio de sección | Ráfaga de viento | — |

</details>

<details>
<summary><b>⛵ Océano</b></summary>

Un velero zarpa al empezar la canción y llega al faro cuando termina.

| Señal | Qué hace |
| --- | --- |
| Avance de la canción | El velero cruza el mar; las velas se despliegan al zarpar |
| Graves | Levantan el oleaje y mecen el velero |
| Bombo | Espuma en la proa |
| Agudos | Destellos en el agua |
| Pulso | Hincha las velas, hace latir el faro y el farol del mástil |
| **Clímax** | Salta un cardumen de peces |
| Cambio de sección | Salta un pez |
| Ánimo | El mismo clima del jardín: sol, atardecer, luna, lluvia o tormenta, con su reflejo en el agua. Con tormenta el mar se pica |
| Carátula | Velas, franjas del faro y banderín |

</details>

<details>
<summary><b>🌆 Ciudad</b></summary>

| Señal | Qué hace |
| --- | --- |
| Avance de la canción | Los edificios suben de a uno; la ciudad queda completa pasada la mitad |
| Graves, medios, agudos | Brillo de las ventanas de los pisos bajos, medios y altos |
| Intensidad | Cuántas ventanas hay encendidas |
| Platillo | Ventanas que se prenden y apagan |
| Pulso | Balizas rojas de las antenas, latido de las ventanas |
| Volumen y tempo | Velocidad del tránsito |
| **Clímax** | Fuegos artificiales |
| Cambio de sección | Un fuego artificial suelto |
| Ánimo | El mismo clima del jardín; con lluvia la calle se moja |
| Canción nueva | La ciudad se hunde y se arma otra distinta |

</details>

<details>
<summary><b>🔥 Fogata</b></summary>

| Señal | Qué hace |
| --- | --- |
| Avance de la canción | El fuego prende al empezar y queda en brasas al final |
| Graves e intensidad | Altura de las llamas |
| Agudos y platillo | Chispas |
| Bombo | Avivan el fuego un instante |
| Pulso | La luz del fuego late sobre los pinos, la carpa y el pasto |
| **Clímax** | Llamarada y una nube de chispas |
| Ánimo | El mismo clima del jardín; el viento inclina las llamas |
| Carátula | Color de las llamas y de la carpa |

</details>

<details>
<summary><b>🚆 Tren nocturno</b></summary>

| Señal | Qué hace |
| --- | --- |
| Avance de la canción | El tren sale al empezar y frena en una estación al final |
| Tempo e intensidad | Velocidad del tren y del paisaje |
| Pulso | Un poste del tendido cruza el centro de la ventana en cada golpe; sus aisladores brillan |
| Medios | Luz de las casas a lo lejos |
| **Clímax** | Pasa un pueblo iluminado |
| Ánimo | El mismo clima del jardín, visto por la ventana |
| Canción nueva | El vidrio se empaña y el viaje vuelve a empezar |

</details>

<details>
<summary><b>🪼 Fondo del mar</b></summary>

| Señal | Qué hace |
| --- | --- |
| Avance de la canción | Van apareciendo medusas, hasta seis |
| Bombo | Las medusas se contraen y se impulsan hacia arriba |
| Agudos y platillo | Burbujas |
| Pulso y volumen | Vaivén de las algas |
| **Clímax** | Todo se enciende con bioluminiscencia y cruza un cardumen |
| Ánimo | De día entran rayos de luz; de noche solo brillan los animales; con tormenta, la superficie se agita |
| Carátula | Color de las medusas |

</details>

<details>
<summary><b>🎆 Fuegos artificiales</b></summary>

| Señal | Qué hace |
| --- | --- |
| Avance de la canción | Pocos fuegos al empezar; en el último 10%, el gran final |
| Pulso e intensidad | Lanzamientos: más seguidos cuanto más intensa la canción |
| Bombo | Fuegos chicos |
| Agudos | Más chispas que crepitan al apagarse |
| **Clímax** | Una tanda de cinco fuegos grandes |
| Cambio de sección | Un anillo |
| Pulso | Los farolitos se mecen y laten |
| Ánimo | Siempre de noche; con ánimos de día queda un resto de crepúsculo. La lluvia y el viento son los del jardín |
| Carátula | Colores de los fuegos y de los farolitos |

</details>

<details>
<summary><b>⛈️ Tormenta en alta mar</b></summary>

| Señal | Qué hace |
| --- | --- |
| Graves e intensidad | La altura de las olas: con calma el mar se mece, con fuerza se levanta |
| Tempo | Qué tan rápido avanzan las olas |
| Pulso | Salpicaduras de espuma desde las crestas |
| Bombo fuerte | A veces, un relámpago |
| **Clímax** | Siempre un relámpago que ilumina todo |
| Canción nueva | Un trueno de bienvenida |
| Pendiente de la ola | El barco sube, baja y se inclina con ella |

</details>

## Cómo detecta los clímax

Las tres bandas se normalizan cuadro a cuadro, así que por sí solas no distinguen un verso
de un coro. Para eso hay una señal aparte, la **intensidad**, que mide el volumen real en
decibeles a escala de canción.

- **Modo archivo: mira el futuro.** Al abrir, un hilo decodifica la canción completa (menos
  de un segundo), mide el volumen cada 0,25 s y lo ubica entre los percentiles 15 y 92 de
  la propia canción. Un clímax es un pasaje que sube sobre 0,72, se sostiene al menos 5 s y
  viene de uno más tranquilo. El instante exacto se afina buscando el mayor salto de
  volumen alrededor. Ver [`src/structure.rs`](src/structure.rs).
- **Modo Spotify: tiempo real.** No hay futuro que mirar. Se comparan dos promedios del
  volumen, uno rápido (~1 s) y uno lento (~18 s). Cuando el rápido salta más de 3 dB sobre
  el lento, cerca del máximo visto hasta ahora, es un clímax. Ver `Dynamics` en
  [`src/analysis.rs`](src/analysis.rs).

## Modo Spotify

```sh
disktui --spotify
```

Abre la app de escritorio de Spotify y pon música antes de lanzarlo.

- **Audio.** Spotify no entrega muestras de audio a terceros, así que se captura lo que
  suena. Al arrancar se prueban tres métodos, un segundo cada uno, y queda el primero con
  señal: dos variantes de tap de Core Audio sobre los procesos de Spotify (solo esa app) y,
  si ninguna sirve, todo el audio del sistema.
- **Título, posición y carátula.** Se leen por AppleScript desde la app. Sin Web API, sin
  OAuth, sin credenciales.
- **Cambio de canción.** La escena se deshace y nace otra con la paleta de la nueva
  carátula: las flores se marchitan, el sistema solar colapsa hacia el sol.

macOS pide dos permisos la primera vez, a nombre de tu terminal: controlar Spotify
(Automatización) y grabar el audio del sistema.

<details>
<summary><b>Si algo no funciona</b></summary>

Con música sonando en Spotify:

```sh
disktui --diagnostico
```

Tarda unos 10 segundos y deja el reporte en `disktui-diagnostico.txt`. Qué hacer según el
resultado:

1. **El título dice `Sin datos de Spotify` o `[1]` da ERROR.** Falta el permiso de
   Automatización: Ajustes del Sistema → Privacidad y seguridad → Automatización → tu
   terminal → Spotify. Sin esto no hay título, carátula ni cambio de canción. El panel de
   la tecla `d` muestra el error exacto de AppleScript.
2. **`[3]` dice SILENCIO en todos.** Falta el permiso de audio: Ajustes del Sistema →
   Privacidad y seguridad → Grabación de pantalla y audio del sistema → sección de solo
   audio del sistema. Agrega tu terminal con `+`, actívala, ciérrala por completo y
   reábrela.
3. **Solo "audio global" da OK.** Funciona, pero captura todo lo que suena en el Mac.
4. **Nada da OK con los permisos puestos.** Instala
   [BlackHole](https://github.com/ExistentialAudio/BlackHole), crea un dispositivo de
   salida múltiple (parlantes + BlackHole) en Configuración de Audio MIDI y usa
   `disktui --spotify --dispositivo BlackHole`.

`--global` se salta los taps por app. `cargo build --no-default-features` compila sin ellos.

</details>

## Arquitectura

```mermaid
flowchart LR
    subgraph audio["Hilo de audio"]
        D[Decoder / captura] --> T[Tap]
        T --> P[Parlantes]
    end
    subgraph fft["Hilo de análisis"]
        Q[Cola a tiempo real] --> F[FFT 2048 + Hann]
        F --> B[3 bandas, golpes, intensidad]
    end
    subgraph tui["Hilo principal"]
        S[Escenas] --> R[ratatui · 60 FPS]
    end
    T -- "ring buffer sin bloqueo" --> Q
    B -- "mpsc" --> S
    M[Pre-análisis de la canción] -. "clímax" .-> S
    C[Carátula → k-means] -. "paleta" .-> S
```

- **Tap.** Un adaptador sobre el `Source` de rodio. Deja pasar el audio intacto y copia una
  mezcla mono a un ring buffer. Rodio pide las muestras justo antes de reproducirlas, así
  que el análisis queda sincronizado con lo que suena.
- **Hilo de análisis.** Consume el ring a ritmo de tiempo real aunque el audio llegue en
  bloques. Aplica ventana de Hann, FFT real y suma la energía por banda.
- **Normalización adaptativa.** Cada banda se divide por su pico reciente, que decae en
  unos 8 segundos. Así una canción baja y una fuerte se ven parecido, y los agudos, que
  traen mucha menos energía que los graves, también mueven la escena.
- **Render en tres capas** sobre la misma grilla: píxeles de media celda (`▀`, color
  completo) para fondo, resplandor, pétalos, sol y planetas; Braille para tallos, hojas,
  órbitas y cometas; glifos (`• ✦ ✧ ❀ ✿ ❁ ░▒▓█`) para polen, estrellas y tierra.
- **Flor.** Rosa polar por capas, `r(θ) = |cos(n·θ/2)|^p`. El exponente baja al abrirse.
  Las semillas del centro siguen una espiral de Fibonacci.
- **Planetas.** Esferas sombreadas según la dirección del sol. Giran según Kepler: los de
  adentro, más rápido.

<details>
<summary><b>Mapa del código</b></summary>

| Archivo | Contenido |
| --- | --- |
| `src/main.rs` | Argumentos, bucle principal, teclado |
| `src/analysis.rs` | FFT, bandas, golpes, intensidad en tiempo real, hilo de análisis |
| `src/features.rs` | Pulso, tonalidad, brillo, densidad y ánimo |
| `src/mood.rs` | Del ánimo al clima |
| `src/structure.rs` | Pre-análisis del archivo: intensidad, clímax, secciones y perfil de toda la canción |
| `src/audio.rs` | Modo archivo: rodio y el tap |
| `src/spotify.rs` | Spotify y Apple Music: AppleScript y carátula |
| `src/capture.rs` | Captura de audio con cpal y orden de respaldos |
| `src/tap_macos.rs` | Tap de Core Audio por proceso |
| `src/diagnose.rs` | Diagnóstico del modo Spotify |
| `src/demo.rs` | Señal sintética |
| `src/session.rs` | Interfaz común de las fuentes de audio |
| `src/scene.rs` | Interfaz común de las escenas |
| `src/garden.rs`, `src/plant.rs`, `src/render.rs` | Escena jardín |
| `src/scenery.rs` | Ambiente del jardín: aurora, luna, cerros, flores silvestres, luciérnagas, mariposas |
| `src/menu.rs` | Menú de inicio y configuración guardada |
| `src/ocean.rs` | Escena océano: mar, velero y faro |
| `src/city.rs` | Escena ciudad: edificios, ventanas y fuegos artificiales |
| `src/campfire.rs` | Escena fogata: fuego, chispas y pinos |
| `src/train.rs` | Escena tren nocturno: paisaje, postes y estación |
| `src/reef.rs` | Escena fondo del mar: medusas, burbujas y algas |
| `src/fireworks.rs` | Escena fuegos artificiales: cohetes, estallidos, lago y farolitos |
| `src/sakura.rs` | Escena cerezos: árbol en flor, pétalos, montaña y faroles |
| `src/lofi.rs` | Escena cuarto lofi: ventana con lluvia, gato, lámpara y tocadiscos |
| `src/aurora.rs` | Escena aurora boreal: cortinas de luz, lago helado y cabaña |
| `src/meadow.rs` | Escena mariposas: pradera y migración en diagonal |
| `src/favorites.rs` | Canciones favoritas |
| `src/transition.rs` | Transiciones entre canciones |
| `src/lyrics.rs` | Letras sincronizadas: LRC, LRCLIB y caché |
| `src/power.rs` | Ahorro de batería: FPS según pausa y batería |
| `src/steady.rs` | No manda a la terminal los cambios de color que no se notan |
| `src/coast.rs` | Escena ruta costera: auto, mar, faroles y gaviotas |
| `src/forest.rs` | Escena bosque: árboles que crecen y florecen |
| `src/concert.rs` | Escena concierto: pantalla LED, luces, parlantes y público |
| `src/alley.rs` | Escena callejón: perspectiva, neón, reflejos en el suelo mojado |
| `src/koi.rs` | Escena estanque: kois, nenúfares y ondas |
| `src/storm.rs` | Escena tormenta: barco, olas, relámpagos y faro |
| `src/karaoke.rs` | Modo karaoke: la línea en letras grandes que se llenan al cantarse |
| `src/sleep.rs` | Temporizador para dormir |
| `src/summary.rs` | Resumen de lo que escuchas (pantalla y póster) |
| `src/moment.rs` | El momento del drop |
| `src/idle.rs` | Pantalla de espera |
| `src/poster.rs` | Póster del herbario y resumen de la semana |
| `src/text.rs` | Texto dentro de imágenes |
| `src/stage.rs` | Todas las escenas juntas: avanzan a la vez |
| `src/tuning.rs` | Correcciones de ánimo por canción |
| `src/herbarium.rs` | Herbario: guardado y vista de la colección |
| `src/postcard.rs` | Postal PNG y grabación en GIF |
| `src/cosmos.rs` | Escena sistema solar |
| `src/canvas.rs` | Lienzo de tres capas |
| `src/palette.rs` | Paleta desde la carátula |
| `src/meta.rs` | Etiquetas y carátula embebida |
| `src/ui.rs` | HUD y barra de reproducción |
| `src/color.rs` | Color RGB, HSL, respaldo a 256 colores |

</details>

## Tests

```sh
cargo test
```

Cubren la FFT por banda, bombos, clímax (en tiempo real y pre-analizado), el crecimiento del
jardín, el orden de aparición del sistema solar, la paleta y el lienzo. Los tests con
archivos de audio reales solo corren si defines `DISKTUI_TEST_MEDIA`; sin ella se saltan.

## Hecho con vibe coding

DiskTUI es un proyecto **vibecoded**: nació y creció conversando con
[Claude](https://claude.ai), el asistente de Anthropic. La idea, las decisiones, qué
probar y qué cambiar vinieron de [@benyens](https://github.com/benyens); el código lo
escribió Claude a partir de esa conversación, una idea a la vez: la flor primero, después
Spotify, el sistema solar, el ánimo, el herbario y el resto de las escenas.

Lo que eso implica, para quien lo lea:

- El código no se escribió a mano línea por línea. Está revisado con tests, `clippy` y
  probándolo, pero puede tener rincones raros.
- Cada parte se verificó como se pudo: tests automáticos, capturas de cada escena y uso
  real en un Mac. El [estado](#estado) dice qué está probado de verdad y qué no.
- Si encuentras algo que no funciona, abre un issue.

## Estado

- **Probado en macOS con Ghostty:** demo, render, menú, modo Spotify (captura solo de la
  app, carátula, cambio de canción), escenas jardín, sistema solar, océano y ciudad.
- **Escrito y probado solo con tests y capturas, todavía sin uso real en macOS:** escena
  automática, panel de detalles, fogata, tren, fondo del mar, fuegos artificiales, ánimo por
  sección, transiciones, momento del drop, póster, GIF vertical, flechas y pantalla de
  espera.
- **Pulso, tonalidad y ánimo:** probados con audio sintético de tempo y acordes conocidos
  (tempo dentro de ±2 BPM, golpes a menos de 60 ms). Con música real pueden fallar, sobre
  todo el ánimo; para eso están `--animo` y las correcciones de la tecla `d`.
- **Apple Music y la fórmula de Homebrew:** escritos sin poder ejecutarlos. Experimentales.
- **Windows:** demo, modo archivo y modo Spotify. El código de Windows se revisó contra
  los tipos reales de su API, pero se escribió sin poder ejecutarlo en Windows.
  Experimental.

## Ideas

- Más escenas: bosque nevado, desierto.
- Binario precompilado en los releases, para instalar sin compilar.
- Listas de reproducción en modo archivo.

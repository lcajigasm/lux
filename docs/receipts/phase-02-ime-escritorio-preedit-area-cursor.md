# Receipt: `lux-platform` — IME de escritorio (preedit, área de cursor)

- Phase: 02
- Crate(s): lux-platform
- Goal: El entregable "IME de escritorio (preedit, área de cursor)" de la
  Fase 2 está completo: `lux-platform` convierte la secuencia cruda de IME
  de `winit` en un estado de composición correcto y consultable (texto en
  composición + rango de cursor, o texto final a insertar), con su gate
  automático en verde y verificación manual humana explícita con IME real
  de macOS.
- Verify: `cargo test -p lux-platform ime::` + verificación manual guiada
  (`cargo run --example ime_smoke -p lux-platform`)
- Iterations: 1 / 10 (parte automática; ver más abajo el histórico de la
  parte manual, que sí necesitó dos vueltas)
- Result: passed
- Opened: 2026-07-30
- Closed: 2026-07-30
- Notes:

  **Qué había ya y qué faltaba.** El entregable 1 (`lux-platform` trait +
  winit) ya convertía `winit::event::Ime` a nuestro propio enum `Ime` y
  reenviaba `set_ime_allowed`/`set_ime_cursor_area` a `winit` de verdad —
  eso ya estaba hecho y cerrado. Lo que faltaba, y es lo que añade este
  entregable, es la lógica real de "qué hacer con esa secuencia": un editor
  necesita saber "esto es texto en composición, muéstralo en su sitio pero
  no lo metas en el buffer todavía" y "esto ya es texto final, insértalo" —
  sin esa capa, cualquier integrador tendría que reinventar la misma
  máquina de estados a mano.

  **`ImeComposition`** (`crates/lux-platform/src/ime.rs`) es esa máquina:
  `Enabled` limpia cualquier composición previa; `Preedit` reemplaza (no
  acumula) el texto y su rango de cursor, y trata un `Preedit` vacío como
  "nada que mostrar" en vez de "componiendo vacío"; `Commit` siempre
  devuelve el texto final y limpia el estado, funcione o no haya habido
  `Preedit` antes (una secuencia de tecla muerta en un solo paso es un caso
  real); `Disabled` **abandona** cualquier composición a medias sin
  convertirla nunca en un commit — si el usuario hace clic fuera a mitad de
  componer, ese texto nunca debe acabar insertado en el buffer.

  **Hallazgo real, verificado por documentación y luego confirmado en
  vivo**: `cursor_range` son **bytes**, no caracteres. El ejemplo que la
  propia documentación de `winit` da (`Ime::Preedit("啊b", Some((3, 3)))`,
  "啊" ocupa 3 bytes) ya lo dejaba claro, y se añadió un test que fija
  exactamente ese caso. La verificación manual (ver abajo) lo confirmó de
  forma independiente con datos reales: "´" (2 bytes en UTF-8) dio
  `cursor_range=(2, 2)`, no `(1, 1)`.

  **Gate automático** (`cargo test -p lux-platform ime::`, 11 tests, 0
  fallos, 0 ignorados, en verde a la primera): además de lo anterior,
  cubre una secuencia japonesa realista (`Enabled` → `Preedit("k")` →
  `Preedit("ka")` → `Preedit("かん")` → `Commit("感じ")`) y una española de
  tecla muerta (`Preedit("´")` → `Commit("é")`) — esta última resultó ser
  exactamente la secuencia que macOS produjo de verdad en la comprobación
  manual, palabra por palabra.

  **Verificación manual — historial real, no solo el resultado final.**
  Primer intento: el usuario ejecutó `cargo run --example ime_smoke -p
  lux-platform` y solo vio eventos de teclado normal (`"€"`, `"e"`), ningún
  `Ime event`. Antes de asumir un bug, se investigó: en teclado español,
  Option+E no es la tecla muerta de acento (da directamente "€"), a
  diferencia de la instrucción inicial que asumía layout "U.S." — ahí
  estaba la causa real, no un fallo de `lux-platform`. Corregida la
  instrucción (usar la tecla de tilde real de la distribución española, la
  misma que el usuario ya usa a diario para escribir en español, en vez de
  adivinar una posición física de tecla), el segundo intento dio la
  secuencia esperada de punta a punta:

  ```
  Ime event: Enabled
  Ime event: Preedit { text: "´", cursor_range: Some((2, 2)) }
    -> composing "´", cursor_range=Some((2, 2))
  Ime event: Preedit { text: "", cursor_range: None }
  Ime event: Commit("é")
    -> committed "é"
  ```

  El usuario confirmó además que el título de la ventana cambió en vivo
  ("composing: ´" y luego "committed: é") — la señal de "preedit visible en
  su sitio" que se usó en vez del pipeline de render de texto (que todavía
  no existe).

  **Veredicto humano explícito** (el propietario del proyecto, verificación
  en vivo con IME real de macOS): confirmado que el título cambió como se
  esperaba, y respuesta afirmativa expresa a "¿lo doy por verificado?" — el
  entregable se da por completo con este veredicto.

  **Fuera de alcance, a propósito, y documentado como laguna real (no
  oculta)**: ningún IME CJK real (japonés/chino) se pudo probar — la
  máquina de turno no tiene esa fuente de entrada disponible y el usuario
  no pudo activarla; queda cubierto solo por el test automático de la
  secuencia japonesa, no por observación humana en vivo. Tampoco se
  confirmó visualmente que la ventana de candidatos del sistema se ancle
  en la posición exacta que reporta `set_ime_cursor_area` — una tecla
  muerta de un solo acento no abre ventana de candidatos (no hay nada que
  posicionar en ese caso), así que no había nada que observar ahí; eso
  necesita una sesión CJK real, para una pasada futura en una máquina que
  la tenga. Windows y Linux siguen sin comprobar, mismo motivo de siempre
  (sin toolchain en esta máquina). "Preedit visible en su sitio" en el
  sentido literal (inline, dentro del texto real del editor) sigue
  pendiente del pipeline de render de `lux-render`, que no existe todavía.
  No se tocó ningún otro crate ni ningún archivo de `lux-platform` fuera de
  `ime.rs`, `lib.rs` (registro del módulo) y el nuevo
  `examples/ime_smoke.rs`.

  `cargo run -p xtask` (fmt, clippy `-D warnings`, `cargo test --workspace`
  — 235 tests de `lux-core` + 23 de `lux-platform` [12 previos + 11
  nuevos] + 21 de `lux-render` + 17 de `lux-ui`, `cargo deny check`,
  límites de 600 líneas/25 campos, aislamiento del núcleo) en verde de
  punta a punta.

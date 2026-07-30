# Receipt: `lux-ui` — Tema Lux Dark

- Phase: 02
- Crate(s): lux-ui
- Goal: El entregable "Tema Lux Dark" de la Fase 2 está completo: existe un
  único tema, Lux Dark, con la identidad de v0 (base índigo-negro
  `#12121B`, acento ámbar dorado `#F0B442`) llevada exactamente al código,
  y su gate — un golden test de captura de tema — pasa sin ningún fallo ni
  test ignorado.
- Verify: `cargo test -p lux-ui theme::` (el prompt maestro no fija un
  comando exacto para este entregable — "golden test de captura de tema" —
  así que elegí este, siguiendo la misma convención `crate::módulo::` que
  todos los entregables anteriores de esta fase; documentado aquí para que
  quede trazado)
- Iterations: 1 / 5
- Result: passed
- Opened: 2026-07-30
- Closed: 2026-07-30
- Notes:

  **Qué significa "captura" aquí.** Igual que en los entregables
  anteriores de esta fase, no existe todavía ningún pipeline de quads/texto
  en `wgpu` que pinte nada en una ventana real — así que no hay framebuffer
  que capturar. Lo que sí es real y capturable: un `Theme` con valores de
  color de verdad, y una función `capture_swatch` que renderiza esos
  colores a un búfer RGBA8 real (relleno plano, sin antialiasing) — el
  mismo formato que `wgpu::Queue::write_texture` esperaría. El golden test
  compara ese búfer, píxel a píxel, contra los valores esperados. Como son
  rellenos planos sin AA, la "tolerancia" correcta es exactamente cero:
  comparación exacta de bytes, no aproximada.

  **Fidelidad al prompt maestro, verificada, no solo transcrita a mano.**
  El test `lux_dark_matches_the_master_prompts_own_hex_codes_verbatim`
  parsea literalmente las cadenas `"#12121B"` y `"#F0B442"` (los mismos
  hex que cita `LUX_PROMPT_MAESTRO.md` §0.6 y el entregable 7) con
  `Color::from_hex` y compara el resultado contra los campos de
  `Theme::LUX_DARK` — así una errata de transcripción al escribir la
  constante en bytes literales (`0x12, 0x12, 0x1B`) la habría pillado el
  test, en vez de fiarme de haberlo copiado bien a ojo.

  **Alcance del resto de la paleta, dejado explícito en el código y aquí**:
  el prompt maestro solo especifica dos colores (base y acento). Un tema
  real necesita al menos un color de primer plano (texto legible sobre el
  fondo), así que añadí `foreground`, `selection` (el acento a alpha
  reducido, para que el texto seleccionado siga siendo legible en vez de
  taparlo del todo) y `muted` (para números de línea / texto atenuado). Se
  documentan explícitamente como completado razonable de este entregable,
  no como parte de la identidad documentada de v0 — para que nadie los
  confunda con valores de espec en una lectura futura. Un tema completo
  basado en tokens/scopes (comentarios, cadenas, corchetes, Lux Light, alto
  contraste) es la Fase 3, no este entregable.

  **Gate automático** (`cargo test -p lux-ui theme::`, 7 tests, 0 fallos, 0
  ignorados, en verde a la primera): los dos hex del prompt maestro
  coinciden con la constante; `from_hex` parsea con y sin `#` inicial, con
  canal alfa explícito, y rechaza entradas mal formadas en vez de
  adivinar; `selection` es realmente `accent` con menos alfa (mismo RGB,
  alfa menor); la captura tiene el tamaño de búfer exacto esperado y cada
  color aparece en la posición de swatch correcta; cada cuadrado de swatch
  es un relleno uniforme sin fuga de color en el borde con el siguiente
  (una comprobación real de que el layout del muestreo no tiene errores de
  desplazamiento de un píxel).

  **Fuera de alcance, a propósito**: pintar el tema en pantalla real
  (necesita el pipeline de quads/texto de `lux-render`, que no existe);
  conectar `Theme` a `editor_view`'s `VisibleLine`/`CursorGlyph`/
  `SelectionRect` (esos tipos todavía no tienen campos de color — cuando
  el pipeline de render exista, esa será la integración real); Lux Light,
  alto contraste y temas basados en tokens/scopes (Fase 3). No se tocó
  ningún otro crate ni ningún archivo de `lux-ui` fuera de `theme.rs` y
  `lib.rs` (registro del módulo) — sin dependencias nuevas.

  `cargo run -p xtask` (fmt, clippy `-D warnings`, `cargo test --workspace`
  — 235 tests de `lux-core` + 23 de `lux-platform` + 21 de `lux-render` +
  32 de `lux-ui` [10 de `editor_view` + 8 de `accessibility` + 7 de `theme`
  + 7 del golden de `editor_view`], `cargo deny check`, límites de 600
  líneas/25 campos, aislamiento del núcleo) en verde de punta a punta.

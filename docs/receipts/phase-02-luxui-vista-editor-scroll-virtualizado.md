# Receipt: `lux-ui` — vista de editor con scroll virtualizado

- Phase: 02
- Crate(s): lux-ui (además de consumir la API pública ya existente de
  lux-core y lux-render, sin modificar ninguno de los dos)
- Goal: El entregable "lux-ui: vista de editor con scroll virtualizado" de
  la Fase 2 está completo: `lux-ui` calcula, para un buffer real y un
  viewport dado, solo la geometría de las líneas visibles (nada fuera de
  rango se rasteriza/da forma), números de línea, un cursor multi-cursor
  con parpadeo, rectángulos de selección multi-línea, guías de
  indentación y reglas de columna. Su gate pasa sin ningún fallo ni test
  ignorado.
- Verify: `cargo test -p lux-ui --test golden_editor_view`
- Iterations: 3 / 12
- Result: passed
- Opened: 2026-07-30
- Closed: 2026-07-30
- Notes:

  **Qué significa "golden" aquí.** No hay pipeline de quads/texto en
  `wgpu` todavía (es la siguiente pieza de `lux-render`, no de este
  entregable), así que no existe framebuffer que comparar píxel a píxel.
  `docs/PERFORMANCE.md` ya define los golden tests de render como
  "capturas... con tolerancia definida" — aquí no hay captura, pero sí
  tolerancia: el test compara la geometría calculada (posiciones x,
  rangos de línea, número de entradas) contra valores esperados con un
  épsilon, exactamente la misma idea aplicada a la capa que sí existe (el
  modelo de vista), no a píxeles que todavía no se pintan. Misma
  honestidad de alcance que ya usaron los dos recibos anteriores de la
  Fase 2.

  **Hallazgo real, no un bloqueo**: `lux_core::Buffer` no tiene ningún
  accessor público por línea (ni `line(idx) -> &str`, ni
  `line_to_char(idx)`) ni un constructor `(línea, columna) -> Position` —
  solo `text()` (documento completo) y `text_range(char_range)`. Como el
  objetivo restringe el trabajo a `lux-ui` exclusivamente ("sin tocar
  nada fuera de su alcance"), no se tocó `lux-core`. En su lugar,
  `editor_view::line_index::LineIndex` (privado a `lux-ui`) construye su
  propio índice de rangos de bytes por línea a partir de `buffer.text()`,
  una vez por frame — coincide exactamente con la convención de
  `Buffer::len_lines()` (una línea final vacía extra tras un `\n` final),
  verificado con un `debug_assert_eq!` en `build_frame` y con tests
  dedicados (incluye el caso CRLF, recortando un `\r` final para que no
  aparezca como glifo suelto). El coste real: ese índice es
  `O(longitud del documento)` por frame, no `O(líneas visibles)` — el
  trabajo caro de verdad (el *shaping* con `parley`) sí queda limitado a
  `visible_line_range`, que es la virtualización que pide el entregable,
  pero el escaneo de líneas en sí no está aún virtualizado. Gap
  documentado en `docs/STATUS.md`, no oculto: la solución correcta es que
  `lux-core` añada un accessor público por línea en una loop futura.

  **Diseño.** `EditorView` envuelve `lux_render::TextLayoutContext` (la
  integración construida en el entregable anterior) más una familia de
  fuente registrada y `EditorViewMetrics` (alto de línea, tamaño de
  fuente, ligaduras, columnas de regla, intervalo de parpadeo).
  `build_frame(buffer, cursors, viewport, elapsed)` hace, en orden:
  1. Calcula `visible_line_range` a partir de `scroll_y`/`height`/
     `line_height`, clamped al total de líneas — la virtualización en
     sí, verificada por construcción (`frame.lines.len() < buffer
     .len_lines()`), no por inspección visual.
  2. Da forma (`TextLayoutContext::layout_line`, reutilizando el
     entregable anterior) **solo** a esas líneas, calcula el número de
     línea alineado a la derecha al ancho de dígitos real, y sus guías de
     indentación.
  3. Recorre `MultiCursor::cursors()`: un cursor vacío que caiga fuera de
     `visible_line_range` no genera nada (un cursor real nunca debería
     dibujarse fuera de la ventana visible); uno visible se mapea a x
     vía `x_for_byte`, que busca el cluster de `lux_render::LineLayout`
     cuyo `byte_range` contiene el offset — la "geometría de clusters
     expuesta para cursor, selección" que el entregable anterior dejó
     lista exactamente para esto. Una selección no vacía multi-línea
     genera un `SelectionRect` por línea visible que cubre, con x
     parcial en sus líneas de inicio/fin y x completa en las intermedias.
  4. Reglas de columna: una única capa de layout de referencia
     (`"0".repeat(columna_máxima)`) da la posición x de cada columna
     pedida — monoespaciado real, no un ancho de carácter adivinado.

  **ponytail**: las guías de indentación tratan cada espacio o tabulador
  inicial como una columna (igual que `Position::column` de `lux-core`
  hoy, que tampoco expande tabs), en vez de expandir tabs a su ancho
  visual — consistente con el resto del código, no un atajo exclusivo de
  aquí. Subir de nivel cuando `lux-config` defina un ancho de tab visual.

  **Lo que el golden test prueba de verdad**
  (`cargo test -p lux-ui --test golden_editor_view`, 7 tests, 0 fallos, 0
  ignorados, sobre un buffer real de 20 líneas):
  - Virtualización: un viewport que solo cabe 4 líneas (`height=60,
    line_height=20`) con `scroll_y=40` da exactamente `visible_line_range
    == 2..6` y `frame.lines.len() == 4`, muy por debajo de las 20 líneas
    reales del documento.
  - Números de línea alineados a la derecha al ancho de dígitos real
    ("20" líneas → 2 dígitos): `[" 3", " 4", " 5", " 6"]`.
  - Multi-cursor con parpadeo: un cursor en pantalla parpadea de verdad
    (visible a 200ms, invisible a 700ms de un intervalo de 500ms) y un
    segundo cursor en la línea 14 (fuera del viewport) no aparece en el
    frame en absoluto.
  - Selección multi-línea: una selección de la columna 4 de la línea 2 a
    la columna 4 de la línea 4 genera 3 rectángulos — x parcial en la
    línea de inicio, ancho completo en la intermedia, x parcial en la
    línea final — con tolerancia de 0.5px sobre aritmética de fuente
    real.
  - Guías de indentación: recuento exacto derivado de espacios iniciales
    reales y `Buffer::indent()` detectado de verdad sobre el texto de la
    prueba, no un valor fijo; una línea sin indentación no genera
    ninguna guía.
  - Reglas de columna: dos reglas (columnas 10 y 20) en orden creciente,
    con la razón entre ambas ≈ 2.0 (monoespaciado real).

  **Iteraciones**: 3. La 1ª falló en dos sitios de `E0502` (el borrow
  checker no permitía pedir `&mut self.text_layout` después de un método
  `&self.line_style(...)` que tomaba prestado todo `self`) — arreglado
  convirtiendo `line_style` en una función libre que solo toma
  `&self.family` por valor, dejando los préstamos de campo disjuntos. La
  2ª iteración compiló y los 7 tests del golden pasaron a la primera,
  pero `cargo clippy -D warnings` marcó un `% 2 == 0` manual
  (`clippy::manual_is_multiple_of`) — cambiado a `.is_multiple_of(2)`. La
  3ª iteración, al correr `cargo deny check` sobre el workspace completo,
  encontró `bans FAILED` por dependencias wildcard: las nuevas
  dependencias de ruta `lux-core`/`lux-render` en `Cargo.toml` no traían
  `version = "0.0.0"`, a diferencia de la convención ya establecida en
  `lux-core-cli`/`benches` — corregido para igualar esa convención.
  Ninguno de los tres fue el mismo fallo repetido dos veces.

  **Fuera de alcance, a propósito**: pintar cualquiera de esto en
  pantalla real (necesita el pipeline de quads/texto de `lux-render`, que
  no existe), tab-expansion visual real en las guías de indentación,
  scroll horizontal más allá de restar `scroll_x` a las x calculadas (no
  ejercitado por el golden test), y tocar `lux-core` para añadir el
  accessor por línea que documenta el hallazgo de arriba. No se tocó
  ningún otro crate.

  `cargo run -p xtask` (fmt, clippy `-D warnings`, `cargo test --workspace`
  — 235 tests de `lux-core` + 12 de `lux-platform` + 21 de `lux-render` +
  17 de `lux-ui` [10 unitarios + 7 del golden], `cargo deny check`,
  límites de 600 líneas/25 campos, aislamiento del núcleo) en verde de
  punta a punta.

# Receipt: Minimapa (entregable 10, último de la Fase 3)

- Phase: 03
- Crate(s): lux-ui
- Goal: El entregable "Minimapa" de la Fase 3 ("Sintaxis, navegación y
  paleta") de `LUX_PROMPT_MAESTRO.md` está completo y su gate pasa sin
  ningún fallo ni test ignorado.
- Verify: `cargo test -p lux-ui minimap::`
- Iterations: 1 / 6
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Alcance.** El master prompt dedica una sola línea a este entregable:
  "Minimapa opcional." — sin criterios de aceptación propios (no aparece
  en la lista de "Criterios de aceptación" de la Fase 3) y sin ningún
  detalle de comportamiento. Con eso de contexto, el diseño se apoyó en lo
  que ya existe en el propio crate en vez de inventar desde cero: igual
  que los cinco entregables anteriores de esta fase, no hay ninguna UI
  interactiva real en `lux-ui` todavía, así que esto es la capa de
  geometría/estado que una UI de minimapa consumiría, siguiendo
  explícitamente el mismo patrón que `EditorView` ya documenta de sí mismo
  ("produces the geometry a render pass would draw, not pixels").
  `docs/STATUS.md` mantiene la fila "Minimap" en `not-started` (regla 1)
  con la prosa detallando lo construido.

  **La decisión de diseño real de este entregable: nunca dar forma a
  glifos.** `EditorView` (Fase 2, entregable 4) ya documenta por qué sólo
  da forma a las líneas visibles: dar forma a un documento de 50k líneas
  entero cada frame sería justo el coste que la virtualización existe para
  evitar. Un minimapa que renderizara texto real de las 50 000 líneas cada
  frame reintroduciría ese mismo coste en silencio, sólo que con otro
  nombre. Por eso `minimap.rs` reduce cada línea a una "barra de
  densidad": desplazamiento de indentación + longitud de contenido, ambos
  contados en caracteres a partir de `LineIndex` (el mismo divisor de
  líneas por byte-range que `EditorView` ya usa, reexportado como
  `pub(crate)` desde `editor_view::line_index` — cero dependencia nueva de
  `lux-render`/shaping). Es la misma técnica real que usan los minimapas
  de producción (una aproximación borrosa del texto, no el texto).

  **Downsampling sin filas vacías al final — no es el approach más
  obvio.** Cuando el documento tiene más líneas que filas de píxel
  disponibles, varias líneas colapsan en una fila. El primer diseño
  (`lines_per_row = ceil(total/available_rows)`, fijar `available_rows`
  filas exactas) deja filas finales vacías cuando la división no es
  exacta — no es un bug de correción (ninguna línea se pierde ni hay
  pánico), pero sí un defecto visual real (el minimapa no llegaría hasta
  abajo). Se consideró distribuir el resto entre cubos (el approach
  "de libro" para repartir N elementos en K cubos sin huecos) pero se
  descartó por ser más código para un entregable sin ninguna
  especificación de comportamiento. La solución elegida: en vez de fijar
  el número de filas a `available_rows`, se calcula cuántas filas hacen
  falta de verdad (`row_count = ceil(total / lines_per_row) <=
  available_rows`) y sólo se generan esas — el minimapa puede usar menos
  filas que el máximo disponible, pero nunca deja una fila a medio
  llenar ni vacía. Verificado explícitamente en
  `build_frame_never_pads_with_empty_trailing_rows`.

  **La fila representa la línea más "densa" del grupo, no la primera ni
  un promedio.** Elegir la primera línea de cada grupo sería más simple
  pero podría esconder una línea larga real si cae después de una línea
  en blanco dentro del mismo cubo. Promediar tampoco tiene sentido para
  una métrica de longitud entera. Se recorre el grupo y se queda con la
  línea de mayor `content_columns` — mismo coste (un `for` sobre el
  grupo, ya necesario para calcular cualquier variante), pero correcto en
  el caso borde que sí importa (una línea larga no debe desaparecer del
  minimapa por mala suerte de bucketing).

  **`indent`/`Indent` se quitó de la firma tras un primer intento.**
  Primera versión pasaba `Buffer::indent()` a `densest_row` "por simetría"
  con `editor_view::indent_guide_x`, pero `indent_columns` cuenta espacios
  en blanco reales de la propia línea, no necesita el estilo/ancho
  detectado del buffer — el parámetro no se usaba en absoluto. Se quitó
  entero (parámetro, import, y la llamada a `buffer.indent()`) en vez de
  dejarlo con un `let _ = indent;` silenciando el aviso; ninguna
  abstracción sin uso real, ni siquiera "por si acaso".

  **`line_for_row`/`row_for_line`: el primitivo de clic-para-navegar.**
  Aunque no hay manejo de eventos de ratón todavía, se expone como
  función pura (`total_lines, available_rows, row -> line`) — el mismo
  patrón ya usado en `search_bar`/`tabs` para primitivos de navegación sin
  UI real todavía: la lógica está lista y probada, sólo falta cablear un
  evento de clic real cuando exista esa capa.

  **Un fallo real durante el desarrollo, no del diseño sino de conteo de
  líneas.** El test `build_frame_never_pads_with_empty_trailing_rows`
  esperaba 2 filas para el buffer `b"1\n2\n"`, pero ese buffer tiene 3
  líneas (el salto de línea final añade una línea vacía extra, la misma
  convención de `ropey`/`LineIndex` que ya documenta ese módulo, y el
  mismo tipo de error de conteo que ya apareció en el receipt de la barra
  de búsqueda de un loop anterior). Corregido tras releer la convención
  documentada en `line_index.rs` en vez de asumir.

  **Verificación real.** `cargo test -p lux-ui minimap::`: 12/12 passed, 0
  ignorados (activado por defecto, `toggle`, `None` cuando está
  desactivado, mapeo 1:1 cuando el documento cabe, downsampling que elige
  la línea más densa, ausencia de filas finales vacías tanto al
  downsamplear como al tener más filas disponibles que líneas, indicador
  de viewport en 1:1 y a través de downsampling, indicador vacío para un
  rango visible vacío, clic-a-línea con caso normal y con fila fuera de
  rango saturada a la última línea, documento vacío como una única fila
  vacía). Un fallo real durante el desarrollo (arriba), corregido y
  reverificado. Gate completo del repo en verde: `cargo run -p xtask`
  (fmt — un ajuste tras escribir el fichero, aplicado con `cargo fmt`;
  clippy `-D warnings` en el workspace entero sin hallazgos; `cargo test
  --workspace`: 235 tests en `lux-core` + el resto del workspace sin
  regresiones, 95 en total en `lux-ui` (83 previos + 12 nuevos); `cargo
  deny check`; límites de fichero — `minimap.rs` en 312 líneas, lejos del
  límite de 600; aislamiento de `lux-core`). Ningún struct nuevo pasa de 3
  campos. No se añadió ninguna dependencia nueva — todo sobre
  `lux_core::buffer::Buffer` y el `LineIndex` ya existente de `lux-ui`.
  Verificado sólo en macOS, misma limitación documentada en el resto de
  `docs/STATUS.md`.

  **Cierre de la Fase 3.** Este es el entregable número 10 y último de la
  lista de la Fase 3 en `LUX_PROMPT_MAESTRO.md`. Los diez tienen ya su
  núcleo/estado implementado y probado (`lux-syntax` para los cinco
  primeros; `lux-ui::goto_anything`, `command_palette`, `search_bar`,
  `tabs`/`status_bar`, y `minimap` para los cinco últimos) — pero
  `docs/STATUS.md` sigue marcando como `not-started` toda fila cuya parte
  de interacción real (UI, eventos de ratón/teclado, wiring de menús,
  render a píxeles) no existe todavía, consistente en toda la fase. Cerrar
  formalmente la Fase 3 (verificar los "Criterios de aceptación" del
  encabezado — presupuestos de reparseo/latencia, corte `v0.2.0` — y
  escribir sus notas de release) no es parte de este entregable concreto
  y no se ha tocado.

  **Qué queda fuera de este entregable** (no tocado): la UI real del
  minimapa (renderizado, manejo de clic/arrastre — no existe ninguna UI de
  interacción en `lux-ui` todavía, mismo estado que dejaron los cinco
  receipts anteriores de esta fase). Se dejó intacto, sin tocar ni
  comitear, el trabajo pendiente de entregables previos que ya estaba en
  el árbol al empezar este loop (auto-indent en `lux-syntax`, y Goto
  Anything / paleta de comandos / barra de búsqueda / pestañas + barra de
  estado de los cuatro loops anteriores, cada uno con su propio receipt,
  aún sin commitear).

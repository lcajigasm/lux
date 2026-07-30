# Receipt: `lux-ui` — `accesskit` conectado

- Phase: 02
- Crate(s): lux-ui
- Goal: El entregable "accesskit conectado" de la Fase 2 está completo: el
  editor se expone como un control `accesskit` de texto multilínea con
  contenido, posición de cursor y selección reales, con su gate automático
  en verde y verificación manual humana explícita con VoiceOver.
- Verify: `cargo test -p lux-ui accessibility::` + verificación manual
  guiada (`cargo run --example accessibility_smoke -p lux-ui`)
- Iterations: 1 / 8 (parte automática, sin contar la investigación previa de
  la API real de `accesskit`/`accesskit_macos`/`accesskit_winit`)
- Result: passed
- Opened: 2026-07-30
- Closed: 2026-07-30
- Notes:

  **Diseño.** `crates/lux-ui/src/accessibility.rs::build_tree` construye un
  `accesskit::TreeUpdate` real: una raíz `Role::MultilineTextInput` con un
  hijo `Role::TextRun` por línea del documento (no solo las líneas
  visibles del scroll virtualizado del entregable 4 — un lector de
  pantalla necesita poder navegar el documento completo). Cada `TextRun`
  lleva `value` (el texto real de la línea), `character_lengths`
  (longitud en bytes de cada "carácter" — la unidad seleccionable más
  pequeña, exactamente lo que ya es cada `ClusterGeometry` de
  `lux_render::text_layout`), `character_positions` y `character_widths`
  (x y avance reales, de la misma pasada de *shaping* que usa el render).
  La raíz referencia sus hijos y expone un `TextSelection` con el
  cursor/selección **primario** de `lux-core` (`MultiCursor::primary()`):
  `accesskit::TextSelection` solo modela un par ancla/foco, igual que todo
  lector de pantalla mainstream — el resto de cursores multi-cursor no
  tiene equivalente en la API de accesibilidad, así que no se exponen (no
  se pierden del estado del editor, solo no viajan a esta capa).

  **Decisión de diseño explícita**: NO se usó
  `parley::layout::accessibility::LayoutAccessibility` (que ya existe y
  hace básicamente lo mismo) porque necesita el `parley::Layout`
  subyacente, que `lux_render::LineLayout` no expone — añadir ese export
  habría sido tocar `lux-render`, un entregable ya cerrado y fuera de
  alcance de este. En su lugar, `build_tree` construye el mismo árbol
  directamente desde `ClusterGeometry`, que ya es público y ya tiene
  exactamente la semántica de "carácter" que `accesskit` necesita.
  Reutiliza `crate::editor_view::line_index::LineIndex` (con su
  visibilidad subida de privada a `pub(crate)`, el único cambio fuera de
  los archivos nuevos) en vez de duplicar la lógica de partir el
  documento en líneas por segunda vez.

  **Hallazgo real durante la construcción del ejemplo de verificación
  manual, antes de escribir ninguna línea de la app de prueba**: conectar
  un adaptador de accesibilidad real a una ventana no es tan simple como
  pasar el handle crudo de la ventana a `accesskit_macos::Adapter::new` —
  esa función NO sub-clasea la vista nativa (NSView) para que el sistema
  operativo le empiece a hacer preguntas de accesibilidad; eso exige
  override real de métodos de `NSAccessibility` vía `objc2`, que es
  precisamente lo que `accesskit_winit` ya resuelve por dentro. Por eso el
  ejemplo de verificación usa `winit` + `accesskit_winit` directamente en
  vez de `lux-platform`: `lux-platform` no tiene wiring de adaptador de
  accesibilidad todavía (es un entregable distinto, ya cerrado, añadirlo
  ahí estaría fuera de alcance), y reimplementar el sub-claseo de NSView a
  mano habría sido reinventar una rueda no trivial que `accesskit_winit`
  ya prueba en producción. Se fijaron las features de `winit` en el
  `Cargo.toml` de `lux-ui` idénticas a las que ya usa `lux-platform`
  (mismo `default-features = false` + lista de features) para que la
  unificación de features de Cargo en un `cargo test --workspace` no
  cambie nada del lado de `lux-platform`.

  **Gate automático** (`cargo test -p lux-ui accessibility::`, 8 tests, 0
  fallos, 0 ignorados, en verde a la primera): un `TextRun` por línea con
  sus hijos en el orden correcto; `value`/`character_lengths` describiendo
  contenido real (la suma de longitudes coincide con los bytes del
  `value`); una línea vacía produce un `TextRun` vacío válido, sin pánico;
  un cursor colapsado mapea a la línea y el índice de carácter correctos;
  un cursor al final de línea usa el recuento de caracteres como índice
  (la semántica exacta que documenta `accesskit::TextPosition`); una
  selección multi-línea tiene ancla y foco en nodos de línea distintos;
  con múltiples cursores activos solo el primario se expone (aunque el
  editor conserva los demás); la raíz siempre es el nodo con foco de su
  propio árbol.

  **Verificación manual — real, de punta a punta.** Ejemplo
  `crates/lux-ui/examples/accessibility_smoke.rs`: ventana real vía
  `winit`, adaptador real vía `accesskit_winit::Adapter::with_direct_handlers`,
  árbol servido por la misma `build_tree` que prueba el gate automático,
  con un documento de 3 líneas real y movimiento de cursor real (flechas
  izquierda/derecha mueven un `Position` de verdad vía
  `next_grapheme`/`prev_grapheme` de `lux-core`, no un simulacro). Confirmé
  que arranca sin fallos antes de entregarlo. Guía dada al propietario del
  proyecto: activar VoiceOver (Cmd+F5), enfocar la ventana, navegar el
  control y sus 3 líneas, mover el cursor con las flechas y confirmar que
  VoiceOver anuncia el cambio.

  **Veredicto humano explícito** (el propietario del proyecto, verificación
  en vivo con VoiceOver real de macOS): *"Todo funcionó como debería"* —
  VoiceOver reconoció el control como campo de texto multilínea, leyó las
  tres líneas de contenido correctamente, y detectó el movimiento del
  cursor. El entregable se da por completo con este veredicto.

  **Fuera de alcance, a propósito, y documentado como laguna real (no
  oculta)**: NVDA y Orca no se pudieron probar (sin entorno Windows/Linux
  en esta máquina, la misma laguna que arrastran todos los entregables de
  esta fase); el ADR 0005 exige los tres lectores de pantalla como criterio
  de aceptación completo de Fase 2, así que esta verificación cubre un
  tercio de ese criterio, no su totalidad. Tampoco existe todavía un
  binario `lux` unificado que conecte `lux-platform`+`lux-render`+`lux-ui`
  de verdad — el ejemplo de verificación es un arnés independiente, no la
  app real (que no existe aún). Exponer más de un cursor/selección a la
  vez no tiene equivalente en la API de accesibilidad y queda
  deliberadamente fuera. No se tocó ningún otro crate ni ningún archivo de
  `lux-ui` fuera de `accessibility.rs`, `lib.rs` (registro del módulo), el
  nuevo `examples/accessibility_smoke.rs`, `Cargo.toml` (dependencias
  nuevas) y una única línea de visibilidad en `editor_view/mod.rs`.

  `cargo run -p xtask` (fmt, clippy `-D warnings`, `cargo test --workspace`
  — 235 tests de `lux-core` + 23 de `lux-platform` + 21 de `lux-render` +
  18 de `lux-ui` [10 de `editor_view` + 8 nuevos de `accessibility`] + 7
  del golden de `editor_view`, `cargo deny check` — sin licencias nuevas,
  `accesskit`/`uuid`/`accesskit_winit` y su cadena ya cubiertos por las
  licencias permisivas existentes —, límites de 600 líneas/25 campos,
  aislamiento del núcleo) en verde de punta a punta.

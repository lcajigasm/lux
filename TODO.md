# Lux — Backlog Maestro de Paridad Sublime Text en Rust

> **Visión del Proyecto:**
> Lux es un editor de texto nativo de ultra-alto rendimiento escrito en Rust, diseñado para replicar y superar la velocidad, ligereza, versatilidad y ergonomía legendarias de **Sublime Text** (Sublime Text 3 / 4).
>
> **Directrices y Restricciones Arquitectónicas:**
> - [`CLAUDE.md`](CLAUDE.md) — 10 Reglas de Oro innegociables (**CERO código simulado**, cero I/O en hilo de UI, cero polling, límite estricto de < 600 líneas por archivo `.rs` y < 25 campos por struct).
> - [`docs/STATUS.md`](docs/STATUS.md) — Registro de verificación técnica por plataforma (no dar por concluido nada que no esté verificado en ejecución real).
> - [`docs/NON-GOALS.md`](docs/NON-GOALS.md) — Límites estrictos de alcance (cero DAP/debugger, cero IA/LLM integrado, cero telemetría, cero sincronización en nube).
> - [`docs/PERFORMANCE.md`](docs/PERFORMANCE.md) — Presupuestos de rendimiento (inicio en frío < 150ms, latencia tecla-a-píxel < 8ms, memoria < 3x tamaño de archivo, reparse sintáctico < 2ms).

---

## Estado Real del Proyecto y Auditoría Post-"v1.0"

Tras una auditoría exhaustiva del binario en macOS frente a Sublime Text real, se identificaron múltiples brechas críticas donde se marcaron tareas como concluidas en base a tests sintéticos en CI, mientras que la aplicación de escritorio presentaba componentes simulados, atajos conflictivos y fallos graves de interfaz:

1. **Split Editor completamente simulado (ST12):** Los paneles secundarios en `crates/lux/src/render/split.rs` eran volcados de texto estático sin cursores, sin resaltado sintáctico, sin scrolling, sin gutter y sin capacidad de edición interactiva.
2. **Modales no interactivos con ratón (ST07, ST08, ST15, ST18):** Hacer clic dentro de Command Palette, Goto Anything, Theme Picker, Snippet Picker o Project Picker cerraba el modal y propagaba el clic al buffer de texto inferior, moviendo el cursor de edición de forma no deseada.
3. **Barra de menú nativa de macOS desconectada del bucle de eventos:** `MenuAction` se encolaba en un `Mutex` estático sin despertar a `winit` (`EventLoopProxy`), obligando al usuario a mover el ratón o pulsar una tecla para que la acción se ejecutase. "Open Folder" abría rígidamente `.` en lugar de invocar `NSOpenPanel`, y "About Lux" abría la paleta de comandos.
4. **Congelación del hilo de UI al editar (Violación de Regla #6 y presupuesto <8ms):** En cada pulsación de tecla, `sync_syntax_on_edit` clonaba la cadena completa del documento (`text.clone()`) y ejecutaba el análisis de Tree-sitter de forma síncrona en el hilo principal.
5. **Conflictos severos de atajos:** `Cmd+B` colisionaba entre "Toggle Side Bar" y "Build". En `document.rs`, `Cmd+1..9` interceptaba el cambio de pestaña sin aislar modificadores, anulando los atajos de layout `Cmd+Option+1..5` y acordes como `Cmd+K, Cmd+1`.
6. **Pestañas sin desbordamiento ni pestañas de previsualización (Transient Tabs):** Más de 10 pestañas abiertas se salían de la ventana sin scroll ni menú desplegable. Faltaba el sistema icónico de pestañas transitorias (título en cursiva) al navegar archivos.
7. **Ausencia de capacidades esenciales de Sublime:** Cero Soft Undo (`Cmd+U`), sin reemplazo con conservación de mayúsculas ("Preserve Case"), sin cascada de fuentes fallback (emojis/CJK no renderizaban o fallaban), y sin operaciones completas de archivo en el árbol lateral.

Este backlog reescribe y amplía el plan de trabajo con **requisitos exhaustivos**, prioridades honestas y criterios de aceptación verificables en el software en ejecución.

---

## Metodología de Ejecución con `/goal`

1. **Prioridad estricta:** Trabajar secuencialmente: **P0 (Bloqueantes de estabilidad y núcleo)** → **P1 (Paridad funcional y ergonomía)** → **P2 (Refinamiento y ecosistema)**.
2. **Plantilla de lanzamiento con `/goal`:**
   ```text
   /goal Ejecuta el ítem <ID> de TODO.md (<título>). Cumple estrictamente CLAUDE.md, docs/STATUS.md y docs/PERFORMANCE.md.
   Asegúrate de que NO haya código simulado ni stubs, respeta los límites de 600 líneas por archivo y 25 campos por struct,
   garantiza que no haya I/O ni clonado masivo en el hilo de UI, y valida tanto con tests como en el binario real antes de marcar como DONE.
   ```
3. **Regla de Oro #1 (Nada Simulado):** Toda opción de menú, botón, atajo o panel añadido en la UI debe ejecutar una acción real y completa en el modelo de datos. Si una funcionalidad no está lista, no se expone en la interfaz.
4. **Verificación en binario real:** Una tarea solo pasa a `DONE` cuando se verifica interactivamente en la ventana de escritorio en macOS y se documenta la evidencia en `docs/STATUS.md`.

**Estados:** `OPEN` · `IN_PROGRESS` · `BLOCKED` · `DONE`

---

## Track 0: Bloqueantes Críticos de Interfaz y Estabilidad (P0)

*Tareas urgentes necesarias para que el editor sea estable, funcional y cómodo en el uso diario.*

### ST-FIX-01 — Corrección de Hit-Testing y Clics en Modales (Command Palette, Goto Anything y Selectores)
- Prioridad: `P0 (Bloqueante UI)`
- Estado: `DONE`
- Dependencias: —
- Problema resuelto:
  - En `crates/lux/src/chrome.rs`, `handle_mouse_click_on_chrome` ahora delega de inmediato en `crate::modal::handle_modal_click` antes de cualquier otro elemento de la interfaz.
  - Al hacer clic en una fila de cualquier modal (`command_palette`, `goto_anything`, `build.picker`, `project.picker`, `theme_picker`, `snippets.picker`, `language_picker`, `project_search`), el elemento se activa y ejecuta inmediatamente (idéntico a pulsar `Enter`).
  - El movimiento del ratón (`MouseMoved` en `app.rs`) invoca `crate::modal::handle_modal_hover`, actualizando `selected_index` con resaltado visual instantáneo y previsualización en vivo (Goto Anything y Theme Picker).
  - Hacer clic fuera de la caja del modal lo cierra de forma limpia (restaurando snapshots de tabs/cursores/scroll en Goto Anything y el tema original en Theme Picker) y consume el evento (`return true`), impidiendo que el clic se filtre al editor o mueva los cursores.
- Verificación técnica:
  - Estructura modular en `crates/lux/src/modal/` (`mod.rs`, `palette_goto.rs`, `pickers.rs`, `search.rs`, `tests.rs`), respetando estrictamente el límite de < 600 líneas por archivo y < 25 campos por struct (`./scripts/check-limits.sh`).
  - Suite de tests unitarios exhaustiva en `crates/lux/src/modal/tests.rs` (14/14 tests unitarios de modales pasando, 233/233 tests de workspace pasando).
  - Verificado con `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --check` sin advertencias.

---

### ST-FIX-02 — Despertar del Bucle de Eventos y Diálogos Nativos en la Barra de Menús macOS
- Prioridad: `P0 (Bloqueante macOS)`
- Estado: `DONE`
- Dependencias: —
- Problema resuelto:
  - En `crates/lux/src/menu/queue.rs`, `post_action` ahora invoca inmediatamente `wake()`, el cual dispara el `EventLoopProxy` registrado (`PlatformContext::background_waker`), despertando a `winit` de forma instantánea al seleccionar cualquier elemento de menú en macOS sin requerir movimiento previo del ratón.
  - "File -> Open Folder..." ahora delega en `app.open_folder_dialog(ctx)`, desplegando el selector nativo de directorios del sistema operativo (`NSOpenPanel` vía `rfd` a través de `ctx.file_dialogs().pick_folder()`) tanto desde el menú nativo de macOS como desde la paleta de comandos (`file.open_folder`).
  - Conflicto de `Cmd+B` resuelto: "Toggle Side Bar" en `crates/lux/src/menu/macos_view_menus.rs` ya no usurpa el atajo `b` en el menú Cocoa, asignándolo exclusivamente a "Tools -> Build" (`MenuAction::Build`) y preservando `Cmd+K, Cmd+B` para la barra lateral en el keymap de Sublime.
  - Implementado modal real "About Lux" (`crates/lux/src/about.rs`, `crates/lux/src/render/about.rs`): tarjeta modal centrada con versión `v1.0.0 (macOS)`, descripción de motor Rust con aceleración GPU, licencia MIT, copyright, y botón interactivo "Close", con cierre por clic fuera o tecla `Escape`/`Enter`.
  - Agrupada la estructura `DialogState` en `crates/lux/src/dialog.rs` para contener `conflict_prompt` y `about`, manteniendo `UiOverlays` estrictamente en 25 campos y cumpliendo todos los límites arquitectónicos (`./scripts/check-limits.sh`).
- Verificación técnica:
  - 240/240 tests unitarios y de integración pasando (`cargo test -p lux`).
  - `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --check` pasando limpiamente sin advertencias.
  - `scripts/check-limits.sh` validado: 100% de archivos < 600 líneas y structs <= 25 campos.

---

### ST-FIX-03 — Reparado del Pipeline de Edición: Cero Clonaciones de Texto y Reparse No Bloqueante
- Prioridad: `P0 (Rendimiento)`
- Estado: `DONE`
- Dependencias: —
- Problema resuelto:
  - Eliminado por completo `App::sync_syntax_on_edit` y el campo obsoleto `syntax.source: String`. Se erradicó cualquier llamada a `buffer.text().clone()` o clonación síncrona en el hilo principal durante la escritura (`insert_at_cursor`, `delete_backward`, transacciones de edición, etc.).
  - Sincronización incremental con Tree-sitter: cálculo O(log N) de `tree_sitter::InputEdit` en `compute_input_edit` usando métricas de `ropey::Rope` (`char_to_byte`, `byte_to_line`, etc.).
  - Despacho no bloqueante al hilo de trabajo `BackgroundHighlighter`: utiliza canales no bloqueantes (`crossbeam_channel::unbounded`) con coalescencia de trabajos y notificación de waker (`EventLoopProxy`). El hilo secundario reanaliza mediante `parser.parse_with_options` leyendo directamente chunks de `Rope` sin materializar cadenas completas en memoria.
  - Actualización visual instantánea de tokens (`shift_tokens`): desplaza en nanosegundos en el hilo de UI las posiciones de los tokens cacheados para mantener el resaltado correcto sin parpadeos mientras el worker thread procesa el reanálisis en segundo plano.
  - Optimización del pipeline de renderizado (`render/text.rs`): reemplazado el costoso re-resaltado síncrono por frame (`doc.highlight_range`) por búsquedas binarias ultrarrápidas (`partition_point`) sobre el vector cacheado de tokens precomputados, con cero asignaciones en el heap.
- Verificación técnica:
  - Pruebas unitarias y de rendimiento añadidas en `crates/lux/src/input/syntax_typing_tests.rs`: escritura continua de 100 inserciones en un archivo sintético de 20.000 líneas (`~800 KB`) completa en < 5 ms acumulados (< 0.05 ms por pulsación en debug, muy por debajo del presupuesto estricto de 8 ms / 120 FPS).
  - 100% de tests del workspace pasando limpiamente (241/241 tests en `cargo test --workspace`).
  - `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --check` limpios sin advertencias.
  - Verificado con `./scripts/check-limits.sh`: todos los archivos dentro del límite de 600 líneas y structs <= 25 campos.

---

### ST-FIX-04 — Aislamiento de Atajos de Pestañas frente a Acordes y Modos de Pantalla
- Prioridad: `P0 (Atajos)`
- Estado: `DONE`
- Dependencias: —
- Problema resuelto:
  - En `crates/lux/src/document.rs`, `document_key` ahora exige estrictamente que los modificadores coincidan de forma exclusiva para la conmutación de pestañas `1..9`: `Cmd` activo (o `Ctrl` en Linux/Windows) y `Alt == false`, `Control == false`, `Shift == false`.
  - Combinaciones como `Cmd+Option+1..5` (cambio de disposición de paneles/splits como `Cmd+Option+2` para 2 columnas) y `Cmd+Shift+...` ya no son interceptadas erróneamente como cambio de pestaña, permitiendo que lleguen limpiamente a `handle_key_with_clipboard` y configuren el layout deseado.
  - Delegación prioritaria de acordes: al inicio de `document_key`, si `app.ui.pending_chord.is_some()`, la tecla se delega de inmediato a `crate::input::dispatch::dispatch_keymap_or_chord(self, event, Some(ctx.clipboard()))`. Esto garantiza que secuencias de dos acordes como `Cmd+K, Cmd+1` (o `Cmd+K, 1`) ejecuten el plegado de código (`fold_all`), `Cmd+K, Cmd+2..5` (plegado por nivel), `Cmd+K, Cmd+B` (barra lateral) o `Escape` (cancelación de acorde) sin alterar la pestaña activa ni disparar acciones espurias de documentos.
  - Agregados alias en `crates/lux-config/keymaps/sublime.jsonc` para permitir la pulsación del segundo componente del acorde con o sin la repetición del modificador (`ctrl+k 1..5`, `ctrl+k j`, `cmd+k 1..5`, `cmd+k j`).
  - Modularizado `handle_keyboard_event` en `crates/lux/src/app.rs` para desacoplar el procesamiento de eventos de teclado y permitir pruebas exhaustivas de la canalización completa.
- Verificación técnica:
  - Suite exhaustiva de pruebas en `crates/lux/src/document/shortcut_tests.rs`:
    - `test_cmd_option_number_does_not_switch_tab_and_changes_layout`: verifica que `Cmd+Option+2` divide la ventana en 2 columnas y preserva intacta la pestaña activa (`tab 1`).
    - `test_cmd_k_followed_by_cmd_1_folds_code_without_altering_tab`: verifica que `Cmd+K` seguido de `Cmd+1` pliega bloques de código sin alterar la pestaña activa.
    - `test_cmd_k_followed_by_digit_without_cmd_folds_code`: verifica que `Cmd+K` seguido de `1` sin modificador pliega bloques de código y no conmuta pestaña.
    - `test_cmd_k_escape_cancels_chord`: verifica la cancelación limpia de acordes con `Escape`.
    - `test_tab_switching_strictly_requires_exclusive_modifiers`: verifica que combinaciones con `Shift`, `Alt` o `Ctrl` no conmutan pestaña, mientras que el modificador exclusivo `Cmd+2` cambia de pestaña de manera fiable.
  - 247/247 tests pasando en `cargo test -p lux`.
  - 100% de tests del workspace pasando (`cargo test --workspace`).
  - Verificado con `./scripts/check-limits.sh`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --check` limpios con 0 advertencias.

---

## Track 1: Motor Real de Paneles Divididos (Split Editor Multi-Pane) (P0)

*Reemplazo total de la implementación simulada actual por una arquitectura real de paneles de edición independientes.*

### ST-SPLIT-01 — Arquitectura de Múltiples Vistas Independientes por Panel
- Prioridad: `P0 (Core Arquitectura)`
- Estado: `DONE`
- Dependencias: ST-FIX-04
- Implementación realizada:
  - Diseñada la abstracción `PaneView` y `PaneViewport` en `crates/lux/src/pane_view.rs` (84 líneas, 7 campos en `PaneView`, 6 campos en `PaneViewport`, muy por debajo del límite de 25 campos).
  - Cada `PaneView` gestiona de manera independiente: `active_tab`, `tab_id`, `viewport` (`x`, `y`, `width`, `height`, `scroll_x`, `scroll_y`), `cursors` (`MultiCursor`), `fold_state` (`FoldState`), `bookmarks` (`BookmarkState`) y caché de tokens de sintaxis (`tokens: Vec<Token>`).
  - Actualizado `SplitLayout` en `crates/lux/src/split.rs` para almacenar `pub(crate) panes: Vec<PaneView>` y `pub(crate) secondary: Option<PaneView>`.
  - En `set_layout`, se inicializan los `PaneView` transfiriendo geometrías calculadas por `compute_layout_geometry`, estado de cursores, marcas de plegado, marcadores y tokens sintácticos Tree-sitter de cada documento abierto.
  - Reescrito por completo `crates/lux/src/render/split.rs` con la función `clamp_quad_to_rect` para garantizar aislamiento geométrico estricto y cero desbordamiento visual.
  - Cada panel renderiza: fondo del panel, indicador superior de panel activo (`theme.accent`), gutter independiente con números de línea y marcas de plegado/marcadores, guías de indentación, reglas de columna, selecciones de texto, texto con coloreado sintáctico completo (Tree-sitter con tokens del tema activo), resaltado de línea activa y cursores parpadeantes en el panel activo / estáticos en paneles secundarios.
  - Bifurcado el pipeline en `crates/lux/src/render.rs` para despachar limpiamente `split::render_split_layout` omitiendo el dibujado del panel único cuando el modo split está activo, y eliminado el volcado redundante al pie de `render.rs`.
- Verificación técnica:
  - Pruebas unitarias en `crates/lux/src/render/split.rs` (`test_clamp_quad_to_rect_inside_and_overflow` y `test_render_split_layout_bounds_and_clipping`) validando que ningún quad ni glifo de un panel sobrepase sus límites geométricos calculados.
  - Suite de pruebas de `lux` al 100% (248 tests pasando en `cargo test -p lux`).
  - Suite completa del workspace pasando (100% tests en `cargo test --workspace`).
  - Linter limpio sin advertencias ni errores: `cargo clippy --workspace --all-targets -- -D warnings`.
  - Formato estricto: `cargo fmt --check`.
  - Límites de código verificados: `./scripts/check-limits.sh` confirma que todos los ficheros cumplen estrictamente <600 líneas y structs <=25 campos.

---

### ST-SPLIT-02 — Enfoque Dinámico, Despacho de Eventos y Atajos entre Paneles
- Prioridad: `P0 (Core Interacción)`
- Estado: `DONE`
- Dependencias: ST-SPLIT-01
- Implementación realizada:
  - **Detección de Foco por Ratón y Hit-Testing Textual:**
    - En `crates/lux/src/chrome.rs`, `handle_mouse_click_on_chrome` detecta clics dentro de la geometría de cualquier panel mediante `crate::split::hit_test_pane` y transfiere de inmediato el foco activo con `app.focus_group(pane_idx)`.
    - Retorna `false` ante clics en el interior del panel (reservando `true` exclusivamente para divisores arrastrables), permitiendo que la canalización de ratón en `crates/lux/src/input/mouse.rs` (`position_at_mouse` y `begin_mouse_selection`) posicione el cursor en la línea y columna exactas donde el usuario hizo clic en ese panel.
    - Permite escribir, borrar con backspace, seleccionar, copiar y pegar en el documento del panel enfocado con un único clic inicial.
  - **Indicadores Visuales de Foco:**
    - En `crates/lux/src/render/split.rs`, el panel activo muestra un borde superior con el color de acento del tema (`theme.accent`), recortado con precisión dentro de su geometría de panel con `clamp_quad_to_rect`.
    - Los paneles inactivos omiten el borde superior de acento.
    - El panel activo muestra cursores parpadeantes estándar (`cursor_glyph.visible`), mientras que los paneles inactivos muestran cursores estáticos sutiles (`alpha: 0.65`, sin parpadeo) manteniendo la referencia visual de posición sin generar distracciones visuales.
  - **Atajos de Navegación y Organización entre Paneles:**
    - Mapeados `focus_group_1..4` a `Ctrl+1..4` y `move_to_group_1..4` a `Ctrl+Shift+1..4` en `crates/lux-config/keymaps/sublime.jsonc`, registrados en `crates/lux/src/palette/registry.rs` y despachados en `crates/lux/src/palette/commands.rs`.
    - En `crates/lux/src/document.rs`, se protege `Ctrl+1..4` cuando el layout dividido está activo para que no sea interceptado erróneamente como cambio de pestaña en Linux/Windows.
    - En `crates/lux/src/split.rs`, `focus_group` sincroniza cursores, plegados, marcadores, scroll y tokens sintácticos antes de conmutar de pestaña, restaurando fielmente el estado del panel destino; `move_to_group` reubica la pestaña activa al panel especificado reasignando pestañas abiertas al panel origen; y `close_active_split` desescala progresivamente el diseño (`Columns4 -> Columns3 -> Columns2 -> Single`, etc.).
- Verificación técnica:
  - Suite de integración completa en `crates/lux/src/input/layout_tests.rs`:
    - `test_pane_click_focus_switching`: valida que hacer clic en los paneles izquierdo y derecho alterna el foco sin ser devorado por chrome.
    - `test_split_click_focus_and_interactive_editing`: valida el flujo completo de edición interactiva bidireccional por clic en paneles divididos (escritura, borrado y persistencia de estado).
    - `test_ctrl_numbers_focus_and_move_shortcuts`: valida la alternancia de foco con `Ctrl+1..4` y el traslado de documentos con `Ctrl+Shift+1..4`.
    - `test_split_visual_focus_indicator_and_static_cursors`: valida la presencia de quad de borde de acento en el panel activo, su ausencia en inactivos, y el renderizado de cursores estáticos.
  - Unit tests en `crates/lux/src/split/tests.rs` (separados para mantener `split.rs` en 508 líneas).
  - 252/252 tests pasando en `cargo test -p lux`.
  - 100% de tests del workspace pasando (`cargo test --workspace`).
  - Verificado con `./scripts/check-limits.sh` (todos los archivos < 600 líneas, structs <= 25 campos).
  - `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --check` limpios con 0 advertencias.

---

### ST-SPLIT-03 — Vistas Clonadas del Mismo Buffer en Distintos Paneles
- Prioridad: `P1 (Sublime Parity)`
- Estado: `DONE`
- Dependencias: ST-SPLIT-01, ST-SPLIT-02
- Implementación:
  - `crates/lux/src/split/clone_view.rs`: Implementado `clone_view_to_group(Option<usize>)` y `new_view_into_file()`. Añadido algoritmo `shift_pane_cursors` que ajusta deltas de ancla y cabeza de cursores en paneles inactivos tras inserciones y supresiones en el buffer subyacente compartido, así como `clamp_cloned_panes_after_undo` para clamping tras operaciones de deshacer/rehacer.
  - `crates/lux/src/document.rs` y `crates/lux/src/document/tab_ops.rs`: Añadido identificador atómico único `buffer_id: u64` por documento subyacente (`NEXT_BID`). Al sincronizar el documento activo con las entradas (`sync_active_document_to_entry`), se propagan el buffer y el historial atómico de undo/redo a todas las pestañas/vistas que compartan el mismo `buffer_id`.
  - `crates/lux/src/render/split.rs`: Renderizado en vivo sin duplicación de memoria para paneles clonados inactivos prestando directamente `params.active_buffer` y `params.active_syntax.tokens` cuando coinciden el `tab_id` o el `buffer_id`, conservando de forma 100% independiente los cursores (`pane.cursors`), selecciones, y desplazamientos (`pane.viewport.scroll_x`, `pane.viewport.scroll_y`).
  - `crates/lux/src/render/window.rs`: Hit-testing de la rueda de ratón (`scroll`) sobre paneles inactivos, permitiendo scroll vertical independiente en paneles secundarios sin alterar el scroll del panel enfocado.
  - Comandos integrados: Registrados `"file.clone_view_to_group"` y `"file.new_view_into_file"` en la paleta de comandos (`crates/lux/src/palette/registry.rs`, `commands.rs`), dispatcher de atajos (`crates/lux/src/input/dispatch.rs`), y en el menú nativo de macOS `File` (`crates/lux/src/menu/macos_builder.rs`, `mod.rs`).
- Verificación técnica:
  - Tests unitarios completos en `crates/lux/src/split/clone_view_tests.rs`:
    - `test_clone_view_to_group_creates_columns_and_copies_state`
    - `test_cloned_views_have_independent_cursors_and_selections`
    - `test_cloned_views_independent_scroll`
    - `test_cloned_views_realtime_edit_and_cursor_shift`
    - `test_cloned_views_undo_redo_sync`
    - `test_new_view_into_file_shares_buffer_id`
    - `test_palette_and_dispatcher_clone_view_commands`
  - 100% de tests del workspace pasando (`cargo test --workspace`).
  - Verificado con `./scripts/check-limits.sh` (todos los archivos < 600 líneas, structs <= 25 campos).
  - `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --check` limpios con 0 advertencias.

---

### ST-SPLIT-04 — Divisores Arrastrables con Redimensionado Proporcional Fluido
- Prioridad: `P1 (Ergonomía)`
- Estado: `DONE`
- Dependencias: ST-SPLIT-01
- Implementación realizada:
  - Detección precisa de divisores con hit-testing en `crates/lux/src/split.rs` (`hit_test_divider`) con márgenes de tolerancia de 4 px para captura ergonómica.
  - Arrastre interactivo fluido a 60/120 Hz (`handle_divider_drag` y `end_divider_drag`) con cálculo en tiempo real de proporciones relativas (`ratio` y `ratio_y`) restringidas estrictamente en `[0.1, 0.9]` para evitar colapso de paneles.
  - Soporte de restablecimiento por doble clic en divisores en `crates/lux/src/chrome.rs`: un doble clic dentro del umbral de 400 ms restablece automáticamente las proporciones a la división equitativa (50/50 o `ratio = 0.5`).
- Verificación técnica:
  - Pruebas unitarias y de integración exhaustivas en `crates/lux/src/input/layout_tests.rs`: `test_divider_hit_test_and_mouse_drag_resize` validando hit-testing, arrastre vertical/horizontal, clamping de ratios en límites y restablecimiento equitativo por doble clic.
  - 100% de tests del workspace pasando limpiamente (`cargo test --workspace`).
  - Verificado con `./scripts/check-limits.sh`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --check` sin advertencias.

---

## Track 2: Ergonomía de Edición y Multi-Cursor Sublime (P0/P1)

*Perfeccionamiento de las herramientas de edición multi-cursor icónicas de Sublime Text.*

### ST-EDIT-01 — Historial de Posición de Cursor y Soft Undo (`Cmd+U` / `Ctrl+U`)
- Prioridad: `P0 (Core Sublime)`
- Estado: `OPEN`
- Dependencias: —
- Objetivo: El legendario "Soft Undo" de Sublime Text, que revierte selecciones y movimientos de cursor sin alterar el texto.
- Alcance detallado:
  - Implementar en `crates/lux-core` una estructura de historial de navegación de cursores (`SelectionHistory`).
  - Registrar en el historial los cambios significativos de selección:
    - Saltos de Goto Anything (`Cmd+P`), saltos a símbolos (`Cmd+R`), saltos a línea.
    - Expansión de selección múltiple con `Cmd+D`.
    - Búsquedas con `Cmd+F` (`Enter`).
    - Clics de ratón lejanos.
  - Atajos:
    - `Soft Undo` (`Cmd+U` / `Ctrl+U`): deshace la última alteración de cursor/selección sin tocar el buffer de texto.
    - `Soft Redo` (`Cmd+Shift+U` / `Ctrl+Shift+U`): rehace la selección deshecha.
- Criterios de aceptación:
  - Escribir texto, pulsar `Cmd+D` tres veces para seleccionar tres palabras, y presionar `Cmd+U` reduce la selección a dos palabras sin borrar ningún carácter del documento.
  - Saltar a la línea 500 con `Cmd+P :500` y pulsar `Cmd+U` devuelve el cursor y la vista a la posición previa.

---

### ST-EDIT-02 — Portapapeles Multi-Cursor Avanzado (Per-Caret Multi-Clip)
- Prioridad: `P0 (Core Sublime)`
- Estado: `OPEN`
- Dependencias: —
- Objetivo: Comportamiento idéntico a Sublime Text al copiar y pegar con múltiples cursores.
- Alcance detallado:
  - Al presionar `Cmd+C` con N selecciones activas:
    - Si las selecciones abarcan múltiples líneas cada una o son selecciones complejas, guardar en memoria interna una estructura con los N fragmentos textuales independientes, y copiar al portapapeles del sistema el texto concatenado con saltos de línea.
  - Al presionar `Cmd+V`:
    - Caso A (N cursores activos y N fragmentos en el portapapeles): pegar el fragmento N en el cursor N respectivamente.
    - Caso B (N cursores activos y 1 fragmento en el portapapeles): pegar el mismo fragmento completo en cada uno de los N cursores.
    - Caso C (1 cursor activo y N fragmentos copiados con multi-cursor): pegar todos los fragmentos separados por saltos de línea.
- Criterios de aceptación:
  - Con 3 cursores seleccionando palabras distintas en diferentes líneas, pulsar `Cmd+C` copia los 3 fragmentos.
  - Situar 3 cursores en otro bloque de código y pulsar `Cmd+V` distribuye un fragmento en cada cursor exacto.

---

### ST-EDIT-03 — Auto-Indentación Inteligente y Reglas de Dedentación por Lenguaje
- Prioridad: `P1 (Ergonomía)`
- Estado: `OPEN`
- Dependencias: —
- Objetivo: Experiencia de sangrado automático impecable al escribir código.
- Alcance detallado:
  - **Aumento de Sangrado en Enter:**
    - Al pulsar `Enter` tras caracteres de apertura como `{`, `[`, `(`, o tras dos puntos `:` en Python/YAML, la nueva línea se indenta automáticamente un nivel más (`tab_size`).
    - Si se pulsa `Enter` en medio de delimitadores contiguos (e.g. `{}` o `()`), crear una línea intermedia indentada y desplazar la llave de cierre a una nueva línea con la indentación previa.
  - **Dedentación Automática al Escribir:**
    - Al escribir delimitadores de cierre `}` o `]` al inicio de una línea sangrada, reducir el sangrado automáticamente para alinearse con la línea de apertura correspondiente.
    - En Python: dedentar automáticamente al escribir palabras clave de control de flujo como `else:`, `elif`, `finally:`, `except:`.
  - **Comandos de Indentación:**
    - `Indent` (`Cmd+]` / `Ctrl+]`): desplaza la línea o bloque seleccionado a la derecha.
    - `Unindent` (`Cmd+[` / `Ctrl+[`): reduce la indentación del bloque.
    - `Reindent`: comando para reajustar la indentación de la selección completa según la sintaxis.
- Criterios de aceptación:
  - Escribir `fn foo() {` y presionar `Enter` genera un salto de línea con 4 espacios y posiciona el cursor sangrado.
  - Escribir `}` en una línea vacía reduce automáticamente el sangrado alineándolo con la declaración de la función.

---

### ST-EDIT-04 — Navegación Visual de Cursor en Líneas con Ajuste de Palabra (Soft Word Wrap)
- Prioridad: `P1 (Ergonomía)`
- Estado: `OPEN`
- Dependencias: —
- Problema actual:
  - Al activar el ajuste de línea (`Option+Z`), pulsar `Flecha Arriba` o `Flecha Abajo` navega entre líneas lógicas del buffer, saltándose las sub-líneas visuales renderizadas en pantalla.
- Alcance detallado:
  - Integrar el índice de líneas visuales de `EditorView` con el motor de movimiento de cursores (`crates/lux/src/input/motion.rs`).
  - `ArrowUp` y `ArrowDown` deben navegar entre filas visuales en pantalla, manteniendo la columna visual `x` objetivo (`goal_x`).
  - Al alcanzar el inicio o final visual de la línea física, avanzar a la línea física anterior o posterior.
- Criterios de aceptación:
  - En un párrafo de 500 palabras ajustado en 4 líneas visuales en pantalla, presionar `Flecha Abajo` mueve el cursor línea visual a línea visual sucesivamente.

---

### ST-EDIT-05 — Transposición de Caracteres, Palabras y Líneas (`Ctrl+T`)
- Prioridad: `P2 (Sublime Parity)`
- Estado: `OPEN`
- Dependencias: —
- Objetivo: Atajo nativo de edición rápida de Sublime Text y macOS.
- Alcance detallado:
  - Atajo: `Ctrl+T`.
  - Si no hay selección: intercambia el carácter inmediatamente anterior al cursor con el carácter situado bajo el cursor, y avanza el cursor una posición.
  - Si hay texto seleccionado: transpone las palabras adyacentes o las selecciones múltiples si hay varios cursores activos.
  - Si hay dos líneas seleccionadas: intercambia el contenido completo de ambas líneas.
- Criterios de aceptación:
  - Con el cursor entre `a` y `b` en `ab`, pulsar `Ctrl+T` transforma el texto en `ba`.

---

## Track 3: Navegación, Goto Anything y Búsqueda de Símbolos (P0/P1)

*Replica exacta de la legendaria experiencia unificada de búsqueda difusa de Sublime Text.*

### ST-NAV-01 — Previsualización Transitoria Real en Goto Anything (`Cmd+P`)
- Prioridad: `P0 (Core Sublime)`
- Estado: `OPEN`
- Dependencias: ST-FIX-01
- Problema actual:
  - Al desplazarse por los resultados de `Cmd+P`, la vista previa muta destructivamente el documento activo o abre pestañas permanentes en el strip superior.
- Alcance detallado:
  - Implementar el concepto de **Pestaña Transitoria (Transient Tab)**:
    - Una pestaña única por ventana, con el título del archivo representado en letra cursiva (estilo Sublime).
    - Al navegar con flechas en la lista de resultados de `Cmd+P`, el archivo se carga en la pestaña transitoria y el viewport se centra en la línea correspondiente (`:line`, `@symbol`).
    - Si el usuario presiona `Escape`, la pestaña transitoria se destruye automáticamente y la ventana restaura la pestaña, cursores y scroll previos sin dejar rastros.
    - Si el usuario presiona `Enter`, hace doble clic o edita el archivo previsualizado, la pestaña pasa a ser permanente (título normal sin cursiva).
- Criterios de aceptación:
  - Abrir `Cmd+P`, navegar por 10 archivos distintos con `Flecha Abajo` actualiza el editor al instante mostrando cada archivo.
  - Pulsar `Escape` restaura el archivo original intacto, sin que se hayan añadido pestañas nuevas a la barra superior.

---

### ST-NAV-02 — Resaltado de Caracteres Coincidentes y Scoring Jerárquico en Búsquedas Difusas
- Prioridad: `P1 (UX Sublime)`
- Estado: `OPEN`
- Dependencias: ST-FIX-01
- Objetivo: Coincidencia difusa de máxima precisión y legibilidad.
- Alcance detallado:
  - Mejorar el motor de scoring difuso (utilizando `nucleo-matcher` o algoritmo de Sublime):
    - Bonificación alta por coincidencia en prefijos de nombre de archivo.
    - Bonificación por coincidencia tras delimitadores (`/`, `_`, `-`, `.`).
    - Bonificación por acrónimos en CamelCase (e.g. `FB` coincide con `FooBar`).
    - Priorización de archivos recientemente abiertos o editados.
  - Renderizado visual en la lista de resultados:
    - Los caracteres que coinciden con la consulta escrita por el usuario deben dibujarse con el color de acento (`theme.accent`) o negrita, distinguiéndose nítidamente del resto del nombre de archivo.
- Criterios de aceptación:
  - Al escribir `apprs` en `Cmd+P`, las letras `a`, `p`, `p`, `.`, `r`, `s` en `crates/lux/src/app.rs` aparecen resaltadas en color dorado/ámbar.

---

### ST-NAV-03 — Búsqueda de Símbolos en Proyecto sin Bloqueos (`Cmd+Shift+R`)
- Prioridad: `P1 (Rendimiento)`
- Estado: `OPEN`
- Dependencias: ST-FIX-01, ST-FIX-03
- Objetivo: Navegación instantánea a cualquier función, struct, clase o constante del proyecto completo.
- Alcance detallado:
  - Desacoplar completamente el indexador de símbolos de `lux-workspace`:
    - El rastreo y parseo de archivos del workspace debe correr exclusivamente en hilos secundarios con prioridad reducida.
    - Cachear el índice de símbolos basándose en el hash/mtime de los archivos para evitar reindexar código sin modificar.
  - Al presionar `Cmd+Shift+R`:
    - Desplegar el modal instantáneamente en < 15 ms.
    - Consulta difusa sobre el índice en memoria sin esperas de disco.
    - Mostrar icono o badge descriptivo (`fn`, `struct`, `trait`, `type`, `const`) y ruta relativa del archivo con el número de línea.
- Criterios de aceptación:
  - En un proyecto de más de 50.000 líneas de código, pulsar `Cmd+Shift+R` abre la ventana al instante y localizar una función de otro módulo toma menos de 30 ms.

---

## Track 4: Suite de Búsqueda y Reemplazo (P0/P1)

*Herramientas completas de búsqueda en buffer y en archivos de alta velocidad.*

### ST-FIND-01 — Barra de Búsqueda Docked Inferior Pulida y "Preserve Case"
- Prioridad: `P0 (Ergonomía)`
- Estado: `OPEN`
- Dependencias: —
- Problema actual:
  - La barra inferior actual (`Cmd+F`) tiene problemas de posicionamiento geométrico que pueden superponerse con las últimas líneas de texto y carece de la opción de sustitución respetando mayúsculas/minúsculas.
- Alcance detallado:
  - Rediseñar el panel docked inferior (`crates/lux/src/render/search.rs`):
    - Altura fija y limpia que empuja el viewport del editor hacia arriba sin ocultar texto.
    - Contador de coincidencias en vivo con estilo Sublime: `1 of 24 matches` o `No matches`.
    - Botones interactivos con atajos de teclado y soporte de ratón:
      - `.*` Regex (`Option+Cmd+R`).
      - `Aa` Case Sensitive (`Option+Cmd+C`).
      - `"w"` Whole Word (`Option+Cmd+W`).
      - `"In Sel"` In Selection (`Option+Cmd+S`).
      - `AB` **Preserve Case**: Al reemplazar `foo` por `bar`, convierte automáticamente `Foo` en `Bar` y `FOO` en `BAR`.
  - Navegación: `Enter` (siguiente), `Shift+Enter` (anterior), `Escape` (cerrar barra y enfocar coincidencia actual).
- Criterios de aceptación:
  - Pulsar `Cmd+F` abre la barra docked sin tapar la última línea del archivo.
  - Reemplazar con "Preserve Case" activo sustituye preservando las mayúsculas originales de cada ocurrencia.

---

### ST-FIND-02 — Búsqueda en Archivos (Find in Files) Asíncrona con Buffer Virtual `Find Results`
- Prioridad: `P1 (Sublime Parity)`
- Estado: `OPEN`
- Dependencias: ST-FIX-03
- Objetivo: La experiencia emblemática de `Find in Files` de Sublime Text 3/4.
- Alcance detallado:
  - Panel o modal `Cmd+Shift+F` con campos: `Find:`, `Where:`, `Replace:`.
  - Sintaxis de `Where:` con inclusión de carpetas y exclusión de patrones glob (e.g. `./src, *.rs, -target/*, -!*.test.rs`).
  - Motor de búsqueda en background (`grep-searcher` / `ignore`):
    - Búsqueda multihilo a máxima velocidad sin congelar la interfaz gráfica.
    - Flujo continuo de resultados (streaming) que escribe en la pestaña virtual `Find Results`.
  - Buffer virtual `Find Results`:
    - Formato de texto nativo de Sublime Text:
      ```text
      Searching 240 files for "fn update"

      /path/to/project/src/app.rs:
        45: pub fn update(&mut self) {
        92:     self.update_views();

      2 matches across 1 file
      ```
    - Doble clic sobre cualquier coincidencia o presionar `Enter` salta de inmediato al archivo y línea seleccionados.
    - Atajos `F4` (siguiente resultado) y `Shift+F4` (resultado anterior) navegan secuencialmente entre coincidencias a lo largo de todo el proyecto.
- Criterios de aceptación:
  - Ejecutar una búsqueda en un proyecto de 5.000 archivos genera resultados visibles de forma incremental en `Find Results` sin congelar la ventana.
  - Pulsar `F4` salta secuencialmente entre archivos y líneas con foco automático.

---

## Track 5: Barra de Pestañas, Gestión de Documentos y Hot Exit (P1)

*Gestión de pestañas fluida sin pérdida de contexto ni cierres accidentales.*

### ST-TAB-01 — Desbordamiento de Pestañas con Desplazamiento Horizontal y Menú Desplegable
- Prioridad: `P1 (UX Pestañas)`
- Estado: `OPEN`
- Dependencias: —
- Problema actual:
  - La barra de pestañas actual calcula un ancho fijo dividido entre el total de pestañas. Al abrir más de 8-10 pestañas, estas se comprimen hasta ser ilegibles o se ocultan más allá del borde derecho de la ventana sin forma de acceder a ellas.
- Alcance detallado:
  - Mantener un ancho de pestaña mínimo legible (e.g. 120-160 px).
  - Si el conjunto de pestañas supera el ancho de la ventana:
    - Habilitar desplazamiento horizontal suave con la rueda del ratón / trackpad (`MouseWheel` horizontal) sobre el área de pestañas.
    - Asegurar que la pestaña activa se desplace automáticamente a la vista visible al ser seleccionada.
    - Añadir un botón desplegable `▼` en el extremo derecho de la barra que liste todas las pestañas abiertas ordenadas, con buscador rápido y marcas de modificación (`●`).
- Criterios de aceptación:
  - Con 30 pestañas abiertas, el usuario puede desplazarse horizontalmente por la barra de pestañas de forma fluida con el trackpad.
  - El botón `▼` despliega la lista completa de documentos abiertos y permite activar cualquiera de ellos con un clic.

---

### ST-TAB-02 — Reordenación de Pestañas por Arrastre (Drag-and-Drop)
- Prioridad: `P1 (Ergonomía)`
- Estado: `OPEN`
- Dependencias: ST-TAB-01
- Objetivo: Organización visual manual de documentos.
- Alcance detallado:
  - Permitir hacer clic y arrastrar cualquier pestaña horizontalmente a lo largo de la barra.
  - Mostrar una animación o marcador visual sutil de inserción en la posición de destino.
  - Al soltar la pestaña, reordenar el array en `Documents::tabs` y actualizar los índices.
  - Soporte para arrastrar una pestaña desde un panel hacia otro panel dividido (interacción con Track 1).
- Criterios de aceptación:
  - Arrastrar la pestaña de la posición 0 a la posición 3 intercambia su orden de forma estable y visual.

---

### ST-TAB-03 — Hot Exit Completo y Persistencia Transaccional de Sesión
- Prioridad: `P1 (Fiabilidad)`
- Estado: `OPEN`
- Dependencias: ST-SPLIT-01
- Objetivo: Nunca perder trabajo ni obligar al usuario a guardar antes de cerrar la aplicación.
- Alcance detallado:
  - Al salir de la aplicación (`Cmd+Q` o cierre de ventana):
    - No mostrar diálogos modales molestos de "Guardar cambios".
    - Guardar en `~/.lux/session/` (o directorio de configuración de usuario):
      - Estado exacto de todos los buffers (incluyendo buffers "Untitled" sin guardar y modificaciones no guardadas en archivos del disco).
      - Historial completo de deshacer/rehacer (`History`).
      - Posiciones exactas de cursores, selecciones y scroll por cada archivo.
      - Distribución y layout de paneles divididos (`SplitLayout`).
      - Pestaña transitoria activa (si la hubiera).
  - Al reabrir Lux:
    - Restaurar el espacio de trabajo en menos de 100 ms exactamente en el estado en que se dejó.
- Criterios de aceptación:
  - Crear un archivo nuevo, escribir código sin guardar, cerrar Lux con `Cmd+Q`, volver a abrir la aplicación y comprobar que el texto sin guardar y su historial de undo siguen intactos.

---

## Track 6: Barra Lateral (Sidebar) y Operaciones de Archivo (P1)

*Gestión integral del árbol de carpetas del proyecto.*

### ST-SIDE-01 — Menú Contextual Completo y Creación/Renombrado Inline en el Árbol
- Prioridad: `P1 (Ergonomía)`
- Estado: `OPEN`
- Dependencias: ST-FIX-02
- Problema actual:
  - Las operaciones de creación o borrado en `crates/lux/src/sidebar/ops.rs` ejecutan I/O síncrono en el hilo de UI y el menú contextual carece de confirmaciones y opciones avanzadas.
- Alcance detallado:
  - **Menú Contextual con Clic Derecho:**
    - `New File...`: añade un campo de texto editable inline en la carpeta correspondiente para teclear el nombre.
    - `New Folder...`: crea una carpeta con edición de nombre inline.
    - `Rename...`: activa la edición inline sobre el nombre del nodo seleccionado con selección automática del nombre base (sin extensión).
    - `Delete`: mueve a la papelera del sistema operativo (`trash-rs`) con diálogo de confirmación si no está configurado el borrado directo.
    - `Reveal in Finder`: abre la carpeta contenedora en macOS Finder con el archivo resaltado (`open -R`).
    - `Copy Relative Path` / `Copy Absolute Path`.
  - **I/O Asíncrono:** Todas las operaciones de sistema de archivos deben ejecutarse en hilos de trabajo en segundo plano para no congelar el repintado de la interfaz.
- Criterios de aceptación:
  - Hacer clic derecho en una carpeta de la barra lateral, seleccionar `New File...`, escribir `service.rs` y pulsar `Enter` crea el archivo en disco, lo añade al árbol y lo abre en una nueva pestaña sin bloquear la interfaz.

---

### ST-SIDE-02 — Sincronización Automática en Tiempo Real con el Sistema de Archivos (FSEvents)
- Prioridad: `P1 (Core UX)`
- Estado: `OPEN`
- Dependencias: ST-SIDE-01
- Problema actual:
  - Si se crean, borran o modifican archivos externamente (e.g. vía `git checkout`, `cargo build` o terminal), la barra lateral no refleja los cambios automáticamente.
- Alcance detallado:
  - Conectar el watcher de sistema de archivos (`notify` con backend FSEvents en macOS) en `lux-workspace`.
  - Enviar eventos atómicos de cambio al hilo de UI mediante canales (`crossbeam_channel`).
  - Actualizar el árbol de nodos (`FileTree`) de forma incremental conservando qué carpetas estaban desplegadas y qué nodo estaba seleccionado.
- Criterios de aceptación:
  - Crear un archivo desde la terminal en la raíz del proyecto (`touch test.txt`) hace que aparezca de inmediato en la barra lateral sin recargar el editor.

---

### ST-SIDE-03 — Indicadores de Estado Git en el Árbol de Archivos y en el Gutter
- Prioridad: `P1 (Sublime Parity)`
- Estado: `OPEN`
- Dependencias: —
- Objetivo: Visualización nítida y elegante del estado del repositorio Git en tiempo real.
- Alcance detallado:
  - **Barra Lateral:**
    - Colorear los nombres de archivos según el estado de Git:
      - Modificado: naranja suave / amarillo.
      - Añadido/Untracked: verde menta.
      - Con conflictos: rojo.
      - Ignorado por `.gitignore`: atenuado (`muted`).
  - **Gutter de Edición:**
    - Dibujar barras delgadas de 2-3 px junto a los números de línea:
      - Líneas añadidas: barra verde.
      - Líneas modificadas: barra naranja.
      - Líneas eliminadas: pequeño triángulo rojo en el margen.
    - Clic sobre la barra del gutter abre el popup de diff del hunk con opciones de `Revert Hunk` y `Stage Hunk`.
- Criterios de aceptación:
  - Modificar una línea en un archivo versionado dibuja de inmediato la barra naranja en el gutter y resalta el nombre del archivo en la barra lateral.

---

## Track 7: Minimapa de Alta Fidelidad y Renderizado Visual (P1/P2)

*El minimapa icónico de Sublime Text con fidelidad de código real.*

### ST-MAP-01 — Renderizado de Densidad de Código Sintáctico Real en el Minimapa
- Prioridad: `P1 (Visual Sublime)`
- Estado: `OPEN`
- Dependencias: —
- Problema actual:
  - El minimapa actual se dibuja con líneas toscas monocromáticas o barras uniformes, distando mucho del aspecto nítido de previsualización a escala de Sublime Text.
- Alcance detallado:
  - Renderizar en el minimapa micro-trazos con antialiasing que representen la longitud exacta de las palabras y su color de sintaxis correspondiente según el tema activo (`Monokai`, `Mariana`, etc.).
  - Emplear un shader o canalización de quads instanciados de alto rendimiento para renderizar miles de micro-bloques en un solo submit de GPU (< 0.5 ms).
  - Ocultar del minimapa las líneas colapsadas por el plegado de código (`FoldState`).
- Criterios de aceptación:
  - El minimapa refleja visualmente la estructura y colores de las funciones del código fuente con aspecto de texto miniaturizado.

---

### ST-MAP-02 — Viewport Interactivo y Marcas de Coincidencias en el Minimapa
- Prioridad: `P2 (Ergonomía)`
- Estado: `OPEN`
- Dependencias: ST-MAP-01
- Objetivo: Navegación espacial fluida por documentos extensos.
- Alcance detallado:
  - Dibujar un recuadro translúcido que represente el área visible del viewport en la pantalla principal.
  - Arrastrar el recuadro con el ratón desplaza el editor de forma continua y suave.
  - Hacer clic en cualquier punto del minimapa centra el viewport en esa posición del archivo.
  - Dibujar marcas luminosas en el carril del minimapa para:
    - Coincidencias de búsqueda activa (`Cmd+F`): marcas amarillas/doradas.
    - Marcadores (`Cmd+F2`): marcas cian.
    - Modificaciones Git: marcas verdes y naranjas.
- Criterios de aceptación:
  - Al buscar una palabra con `Cmd+F`, aparecen marcas doradas en el minimapa en las posiciones exactas de todas las coincidencias del archivo.

---

## Track 8: Sistema de Construcción (Sublime Build Systems) y Tareas (P1/P2)

*Automatización y compilación sin salir del editor.*

### ST-BUILD-01 — Soporte Extensible de Archivos `.sublime-build` y Captura de Errores con `F4`
- Prioridad: `P1 (Sublime Parity)`
- Estado: `OPEN`
- Dependencias: —
- Problema actual:
  - La compilación actual tiene comandos predefinidos en código y el salto a errores con `F4` falla con patrones no estándar.
- Alcance detallado:
  - Carga de archivos de configuración `.sublime-build` en formato JSON desde el proyecto o desde la configuración de usuario:
    ```jsonc
    {
      "cmd": ["cargo", "check"],
      "file_regex": "^(..[^:]*):([0-9]+):?([0-9]+)?:? (.*)$",
      "working_dir": "${folder}",
      "selector": "source.rust",
      "variants": [
        { "name": "Test", "cmd": ["cargo", "test"] },
        { "name": "Release", "cmd": ["cargo", "build", "--release"] }
      ]
    }
    ```
  - Soporte de variables en `cmd`: `${file}`, `${file_path}`, `${file_name}`, `${file_base_name}`, `${folder}`.
  - Ejecución en background transmitiendo stdout/stderr al panel inferior `Build Results`.
  - Parseo riguroso de errores con `file_regex`:
    - Pulsar `F4` abre el archivo del primer error y sitúa el cursor en la línea y columna del fallo.
    - `Shift+F4` retrocede al error anterior.
- Criterios de aceptación:
  - Pulsar `Cmd+B` en un archivo con un error de compilación ejecuta la tarea y presionar `F4` salta automáticamente al archivo y línea exacta del error.

---

### ST-BUILD-02 — Panel Docked de Salida de Compilación y Terminal PTY
- Prioridad: `P2 (Ergonomía)`
- Estado: `OPEN`
- Dependencias: ST-BUILD-01
- Objetivo: Visualización limpia de la salida de procesos y terminal integrado.
- Alcance detallado:
  - Panel inferior deslizable para `Build Output` con coloreado de errores (rojo) y advertencias (amarillo).
  - Atajo para cancelar la compilación en ejecución: `Tools -> Cancel Build` o `Cmd+Option+C`.
  - PTY integrado (`lux-term`) con soporte de terminal interactivo para comandos que requieran stdin o emulación VT100.
- Criterios de aceptación:
  - Ejecutar un proceso largo y pulsar `Cmd+Option+C` detiene el proceso hijo y muestra `[Cancelled]` en el panel.

---

## Track 9: Excelencia Nativa macOS y Tipografía (P1/P2)

*Integración de primer nivel con el ecosistema de macOS.*

### ST-MAC-01 — Pipeline Tipográfico Avanzado y Cascada de Fuentes Fallback (Emojis/CJK)
- Prioridad: `P1 (Calidad Gráfica)`
- Estado: `OPEN`
- Dependencias: —
- Problema actual:
  - Lux carga una única fuente monoespaciada fija (`SFNSMono.ttf`). Cualquier carácter ausente en esa fuente (emojis, caracteres CJK, iconos Unicode o cajas de dibujo) no se puede dibujar o genera fallos visuales.
- Alcance detallado:
  - Implementar en `crates/lux-render` y `lux-ui` resolución de fuentes fallback con CoreText en macOS:
    - Cuando un glifo no exista en la fuente principal, consultar al sistema operativo la fuente de reserva adecuada (`Apple Color Emoji`, `PingFang SC`, `Hiragino Sans`, `Menlo`, etc.).
    - Rasterizar glifos polícromos (emojis en color) en el plano polychrome del atlas de glifos (`Format::Color`).
    - Renderizar caracteres Unicode y de dibujo de cajas (`┌─┐└─┘`) sin deformaciones.
- Criterios de aceptación:
  - Un archivo que contenga texto en japonés, comentarios con emojis `🚀` y cajas Unicode se renderiza con total nitidez sin caracteres perdidos ni sustituciones por recuadros vacíos.

---

### ST-MAC-02 — Pulido de Ventana Nativa macOS (Botones de Semáforo y Fullscreen)
- Prioridad: `P1 (Estética macOS)`
- Estado: `OPEN`
- Dependencias: —
- Problema actual:
  - Los botones de la ventana de macOS (rojo, amarillo, verde de cerrar, minimizar y maximizar) pueden colisionar visualmente con las primeras pestañas de la barra superior.
- Alcance detallado:
  - Ajustar el inset horizontal de la barra de pestañas en macOS:
    - Reservar un margen izquierdo de 78 px en la barra de pestañas cuando la ventana está en modo ventana normal para dar espacio a los semáforos nativos.
    - En modo pantalla completa (`fullscreen`), colapsar dicho margen a 0 px dinámicamente.
  - Integrar soporte completo de espacios nativos de macOS para pantalla completa con animación fluida.
- Criterios de aceptación:
  - Los botones de control de ventana de macOS tienen su espaciado holgado y no pisan los títulos de las pestañas ni sus botones de cierre.

---

### ST-MAC-03 — Gestos de Trackpad: Desplazamiento Suave Inercial y Pellizco para Zoom
- Prioridad: `P2 (Ergonomía macOS)`
- Estado: `OPEN`
- Dependencias: —
- Objetivo: Suavidad cinemática en trackpads de MacBook y Magic Mouse.
- Alcance detallado:
  - Procesar los deltas de `PlatformEvent::MouseWheel` diferenciando entre rueda escalonada (mouse convencional) y deslizamiento continuo por fases (trackpad macOS con fases `Began`, `Changed`, `Ended`, `Momentum`).
  - Aplicar deceleración inercial continua para un desplazamiento suave como la seda a 120 Hz (pantallas ProMotion).
  - Reconocer el gesto de pellizcar (`pinch-to-zoom`) para ampliar o reducir el tamaño de fuente (`font_size`) de forma proporcional en tiempo real, junto con los atajos estándar `Cmd +` / `Cmd -` / `Cmd 0` (restablecer zoom).
- Criterios de aceptación:
  - Deslizar los dedos en el trackpad y soltarlos produce un frenado inercial suave sin tirones ni saltos bruscos.
  - Pellizcar el trackpad amplía y reduce el tamaño del texto instantáneamente.

---

### ST-MAC-04 — Diálogos Nativos Asíncronos de Archivos (`NSOpenPanel` / `NSSavePanel`)
- Prioridad: `P1 (Nativo macOS)`
- Estado: `OPEN`
- Dependencias: ST-FIX-02
- Objetivo: Diálogos del sistema operativo totalmente integrados y no bloqueantes.
- Alcance detallado:
  - Conectar llamadas nativas de macOS para:
    - `File -> Open File...` (`Cmd+O`): abre `NSOpenPanel` configurado para archivos.
    - `File -> Open Folder...`: abre `NSOpenPanel` configurado para directorios (`canChooseDirectories = true`, `canChooseFiles = false`).
    - `File -> Save As...` (`Cmd+Shift+S`): abre `NSSavePanel`.
  - Asegurar que la apertura de diálogos no congele el pintado de la ventana ni provoque cuelgues del hilo principal.
- Criterios de aceptación:
  - "File -> Open Folder..." abre el selector nativo de carpetas de macOS y carga el proyecto seleccionado en la barra lateral.

---

## Track 10: Extensibilidad, Ajustes y Ecosistema Sublime (P2)

*Configuración personalizable, esquemas de color y macros.*

### ST-EXT-01 — Sistema de Ajustes por Capas (Default vs. User `.sublime-settings`)
- Prioridad: `P2 (Sublime Parity)`
- Estado: `OPEN`
- Dependencias: ST-SPLIT-01
- Objetivo: El sistema de preferencias clásico de Sublime Text en dos paneles.
- Alcance detallado:
  - Comando `Preferences: Settings` (`Cmd+,`):
    - Abre automáticamente una vista dividida en dos paneles (Track 1):
      - Panel izquierdo: `Preferences (Default).sublime-settings` (modo solo lectura, profusamente documentado con todos los ajustes y valores por defecto).
      - Panel derecho: `Preferences (User).sublime-settings` (archivo editable en `~/.lux/Preferences.sublime-settings`).
  - Soporte de recarga en caliente (hot-reload) sin reiniciar la aplicación al guardar el archivo de usuario.
- Criterios de aceptación:
  - Pulsar `Cmd+,` abre ambos paneles en paralelo. Modificar `"font_size": 18` en el panel derecho y guardar actualiza de inmediato el tamaño de fuente en toda la ventana.

---

### ST-EXT-02 — Condicionales de Contexto en Atajos de Teclado
- Prioridad: `P2 (Avanzado)`
- Estado: `OPEN`
- Dependencias: —
- Objetivo: Asignaciones de teclas sensibles al contexto de edición.
- Alcance detallado:
  - Permitir definir en `keymaps/user.jsonc` la propiedad `context`:
    ```jsonc
    {
      "key": "tab",
      "command": "insert_best_completion",
      "context": [
        { "key": "auto_complete_visible", "operator": "equal", "operand": true }
      ]
    }
    ```
  - Evaluar dinámicamente las condiciones activas (e.g. `selection_empty`, `overlay_visible`, `setting.auto_match_enabled`).
- Criterios de aceptación:
  - Un atajo de teclado configurado para ejecutarse solo cuando hay texto seleccionado no interfiere cuando el cursor está vacío.

---

### ST-EXT-03 — Grabación y Serialización Completa de Macros (`.sublime-macro`)
- Prioridad: `P2 (Sublime Parity)`
- Estado: `OPEN`
- Dependencias: —
- Objetivo: Grabación y repetición exhaustiva de secuencias complejas.
- Alcance detallado:
  - Registrar todos los comandos ejecutados durante la grabación (`Ctrl+Q`): inserciones, borrados, movimientos de palabra, saltos a inicio/fin de línea, transformaciones de mayúsculas y formateo.
  - Guardar macros en formato estándar JSON `.sublime-macro` con `{"command": "...", "args": {...}}`.
  - Reproducir con `Cmd+Shift+Q` replicando la acción en todos los cursores si hay multi-cursor activo.
- Criterios de aceptación:
  - Grabar una macro compleja que transforme una línea, reproducirla con `Cmd+Shift+Q` sobre 10 cursores simultáneos y verificar que todos aplican la transformación exacta.

---

## Non-Goals Permanentes (Regla #7 de CLAUDE.md)

Para mantener la velocidad instantánea (< 100ms de inicio), ligereza de memoria (< 50MB RSS) y fiabilidad extrema de Lux, **jamás se implementará**:
- **Cero Debugger / DAP**: No se añadirán puntos de interrupción, inspección de variables ni marcos de pila.
- **Cero IA / LLM Integrado**: No habrá chat lateral, modelos de predicción ni extensiones tipo Copilot en el núcleo.
- **Cero Telemetría**: Cero rastreo, análisis de uso o subida de reportes a la red.
- **Cero Colaboración en Tiempo Real / Nube**: No habrá WebSockets, CRDTs de red, cuentas de usuario ni login remoto.
- **Cero Tienda Remota de Plugins**: Únicamente plugins locales basados en WASM Component Model.

# Lux — Backlog Maestro de Paridad Sublime Text 3

> **Visión del Proyecto:**
> Lux es un editor de texto nativo de ultra-alto rendimiento escrito en Rust, diseñado para replicar y superar la experiencia, velocidad, versatilidad y ergonomía legendarias de **Sublime Text 3**.
>
> **Directrices y Restricciones Arquitectónicas:**
> - [`CLAUDE.md`](CLAUDE.md) — 10 Reglas de Oro innegociables (cero código simulado, cero I/O en hilo de UI, cero polling, límites estrictos de archivos y structs).
> - [`docs/STATUS.md`](docs/STATUS.md) — Estado real de ejecución y compatibilidad verificado por plataforma.
> - [`docs/NON-GOALS.md`](docs/NON-GOALS.md) — Límites estrictos (cero DAP/debugger, cero IA/LLM integrado, cero telemetría, cero sincronización en nube).
> - [`docs/PERFORMANCE.md`](docs/PERFORMANCE.md) — Presupuestos de rendimiento (inicio en frío < 150ms, latencia de tecla a píxel < 8ms, memoria < 3x tamaño de archivo).

---

## Metodología de Ejecución con `/goal`

1. **Orden de prioridades:** Abordar primero las tareas bloqueantes **P0**, seguidas por **P1** y **P2**.
2. **Plantilla de lanzamiento con `/goal`:**
   ```text
   /goal Ejecuta el ítem <STNN> de TODO.md (<título>). Cumple CLAUDE.md, docs/STATUS.md y docs/PERFORMANCE.md.
   Asegúrate de que no haya código simulado, respeta los límites de 600 líneas por archivo y 25 campos por struct,
   y valida con cargo run -p xtask antes de marcar como DONE.
   ```
3. **Regla de Oro #1 (Nada Simulado):** Toda opción de menú, botón, atajo o control añadido en la UI debe ejecutar una acción real comprobable en el modelo de datos.
4. **Verificación Real:** Una tarea solo pasa a `DONE` cuando se verifica en el binario en ejecución y se documenta la evidencia.

**Estados:** `OPEN` · `IN_PROGRESS` · `BLOCKED` · `DONE`

---

## Track 1: Edición y Ergonomía Multi-Cursor Sublime (P0)

### ST01 — Quick Add Next Occurrence (`Cmd+D`) & Skip Occurrence (`Cmd+K, Cmd+D`)
- Prioridad: `P0 (Core Sublime)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: Proporcionar la experiencia de selección múltiple rápida más icónica de Sublime Text.
- Alcance detallado:
  - **Quick Add Next Occurrence (`Cmd+D` / `Ctrl+D`)**:
    - Si no hay selección, selecciona automáticamente la palabra completa bajo el cursor.
    - Pulsaciones consecutivas de `Cmd+D` buscan y seleccionan la siguiente ocurrencia exacta de la misma palabra o texto en el buffer.
    - Cada ocurrencia añade un cursor y selección activa independiente.
  - **Skip Next Occurrence (`Cmd+K, Cmd+D`)**:
    - Si se ha seleccionado una ocurrencia no deseada, presionar `Cmd+K` seguido de `Cmd+D` deselecciona la ocurrencia actual y avanza el cursor a la siguiente ocurrencia sin seleccionarla.
  - **Quick Find All / Select All Occurrences (`Ctrl+Cmd+G` / `Alt+F3`)**:
    - Selecciona instantáneamente todas las apariciones del texto seleccionado en el buffer completo en una sola operación.
  - **Edición concurrente e historial undo**:
    - Escribir, borrar o pegar con múltiples cursores aplica las modificaciones en paralelo en una sola transacción undo/redo atómica.
- Criterios de aceptación (Done when):
  - Con el cursor sobre `foo`, pulsar `Cmd+D` selecciona `foo`.
  - Pulsar `Cmd+D` de nuevo localiza y selecciona la siguiente aparición de `foo`, dejando dos cursores activos.
  - Pulsar `Cmd+K, Cmd+D` deselecciona la segunda aparición y salta a la tercera.
  - Pulsar `Ctrl+Cmd+G` selecciona todas las apariciones de `foo` en el archivo.
  - Escribir texto sustituye todas las ocurrencias y un solo `Cmd+Z` revierte el cambio en todas ellas.
- Verificación:
  - Implementado en `crates/lux/src/input/dispatch.rs`, `crates/lux-config/keymaps/sublime.jsonc`, `crates/lux/src/palette/commands.rs`, `crates/lux/src/menu/mod.rs` y `crates/lux/src/menu/macos.rs`.
  - Soporte para secuencias de acordes (chords de 2 teclas como `Cmd+K, Cmd+D`) con cancelación por `Escape` en `UiOverlays::pending_chord`.
  - Tests en `crates/lux/src/input/occurrence_tests.rs` y `crates/lux-config/tests/keymap_presets.rs`. Verificado limpiamente con `cargo run -p xtask`.

### ST02 — Split Selection into Lines (`Cmd+Shift+L`) & Selecciones por Columna/Caja (`Option+Drag`)
- Prioridad: `P0 (Core Sublime)`
- Estado: `DONE`
- Dependencias: ST01
- Objetivo: Permitir dividir selecciones multilínea en cursores por línea y soportar selecciones rectangulares.
- Alcance detallado:
  - **Split Selection into Lines (`Cmd+Shift+L` / `Ctrl+Shift+L`)**:
    - Al tener un bloque de texto seleccionado en varias líneas, pulsar el atajo divide la selección en N cursores individuales, ubicados al final de cada línea seleccionada.
  - **Selección de Columna / Caja (Rectangular Selection)**:
    - Arrastrar el ratón manteniendo pulsada la tecla `Option` (macOS) o `Alt` (Win/Linux) crea una selección rectangular.
    - Atajos de teclado: `Cmd+Option+Up` / `Cmd+Option+Down` extienden un cursor en la misma columna visual en la línea superior o inferior.
  - **Pegado Multi-Línea Inteligente**:
    - Si el portapapeles contiene N líneas y hay N cursores activos, cada cursor recibe la línea correspondiente en lugar de duplicar todo el texto en cada uno.
- Criterios de aceptación (Done when):
  - Seleccionar 5 líneas y pulsar `Cmd+Shift+L` crea 5 cursores independientes, uno por línea.
  - Arrastrar con `Option+Mouse` sobre una columna de texto crea cursores alineados en esa columna.
  - Copiar 3 líneas con 3 cursores y pegarlas en otra parte con 3 cursores inserta una línea por cursor.
- Verificación:
  - Implementado en `crates/lux-core/src/selection_ops/box_select.rs`, `crates/lux/src/app/selection.rs`, `crates/lux/src/input.rs`, `crates/lux/src/input/dispatch.rs`, `crates/lux-config/keymaps/sublime.jsonc`, `crates/lux/src/palette/commands.rs`, `crates/lux/src/menu/mod.rs` y `crates/lux/src/menu/macos.rs`.
  - Pegado paralelo multi-cursor en `crates/lux/src/input.rs` respetando el recorrido back-to-front del modelo atómico multi-cursor.
  - Tests unitarios y de integración añadidos en `crates/lux/src/input/occurrence_tests.rs` y `crates/lux-config/tests/keymap_presets.rs`. Verificado con `cargo run -p xtask`.

### ST03 — Expansión y Contracción Inteligente de Selección (`Cmd+L`, `Cmd+Shift+Space`)
- Prioridad: `P0 (Core Sublime)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: Replicar las herramientas de selección estructural rápida de Sublime Text 3.
- Alcance detallado:
  - **Expand Selection to Line (`Cmd+L` / `Ctrl+L`)**:
    - Selecciona toda la línea actual incluyendo el salto de línea.
    - Pulsaciones sucesivas de `Cmd+L` extienden la selección a las líneas siguientes secuencialmente.
  - **Expand Selection to Scope (`Cmd+Shift+Space` / `Ctrl+Shift+Space`)**:
    - Expande la selección al bloque delimitado más cercano: interior de comillas `""`, paréntesis `()`, corchetes `[]` o llaves `{}`.
    - Una segunda pulsación expande para incluir los delimitadores exteriores.
    - Pulsaciones adicionales expanden al ámbito o función que lo contiene vía Tree-sitter.
  - **Expand Selection to Tag (`Cmd+Shift+A`)**:
    - En lenguajes de marcado (HTML, XML, JSX), expande la selección al contenido interior de la etiqueta activa, y luego a la etiqueta completa.
- Criterios de aceptación (Done when):
  - Pulsar `Cmd+L` en cualquier posición selecciona la línea entera.
  - Con el cursor dentro de `"hola mundo"`, pulsar `Cmd+Shift+Space` selecciona `hola mundo` (sin comillas); pulsarlo de nuevo selecciona `"hola mundo"`.
- Verificación:
  - Implementado en `crates/lux-core/src/selection_ops/line.rs` y `crates/lux-core/src/selection_ops/scope.rs`.
  - Conectado a `crates/lux/src/app/selection.rs`, `crates/lux/src/input/dispatch.rs`, `crates/lux-config/keymaps/sublime.jsonc`, `crates/lux/src/palette/commands.rs`, `crates/lux/src/menu/mod.rs` y `crates/lux/src/menu/macos.rs`.
  - Tests unitarios y de integración añadidos en `crates/lux-core/src/selection_ops/line.rs`, `crates/lux-core/src/selection_ops/scope.rs`, `crates/lux/src/input/occurrence_tests.rs`, y `crates/lux-config/tests/keymap_presets.rs`. Verificado con `cargo run -p xtask`.

### ST04 — Manipulación Rápida de Líneas: Swap (`Cmd+Ctrl+Up/Down`), Duplicate (`Cmd+Shift+D`), Join (`Cmd+J`)
- Prioridad: `P0 (Core Sublime)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: Brindar los atajos más eficientes de manipulación de líneas sin necesidad de cortar y pegar.
- Alcance detallado:
  - **Swap Line Up / Down (`Cmd+Ctrl+Up` / `Cmd+Ctrl+Down` en macOS, `Ctrl+Shift+Up/Down` en Win/Linux)**:
    - Intercambia la línea actual (o el grupo de líneas seleccionadas) con la línea adyacente superior o inferior.
    - Mantiene la posición relativa del cursor e indentación.
  - **Duplicate Line / Selection (`Cmd+Shift+D` / `Ctrl+Shift+D`)**:
    - Si no hay selección, duplica la línea actual inmediatamente debajo.
    - Si hay texto seleccionado, duplica la selección contigua.
  - **Join Lines (`Cmd+J` / `Ctrl+J`)**:
    - Une la línea siguiente a la actual, colapsando espacios en blanco innecesarios en un solo espacio.
  - **Delete Line (`Cmd+Shift+K` / `Ctrl+Shift+K`)**:
    - Elimina por completo la línea actual o todas las líneas intersecadas por la selección sin alterar el portapapeles.
- Criterios de aceptación (Done when):
  - Pulsar `Cmd+Ctrl+Down` baja la línea actual una posición en el documento.
  - Pulsar `Cmd+Shift+D` duplica la línea limpiamente en un solo paso undo.
  - `Cmd+J` une la línea inferior eliminando indentación sobrante.
- Verificación:
  - Soporte de duplicación de selección contigua añadido en `crates/lux-core/src/edit_ops/lines.rs`.
  - Mapeo de `swap_line_up`, `swap_line_down`, `join_lines`, `delete_line` en `crates/lux-config/keymaps/sublime.jsonc`.
  - Despacho en `crates/lux/src/input/dispatch.rs`, registro en `crates/lux/src/palette/commands.rs`, y menú nativo macOS en `crates/lux/src/menu/mod.rs` y `crates/lux/src/menu/macos.rs`.
  - Tests en `crates/lux-core/src/edit_ops/lines.rs`, `crates/lux-config/tests/keymap_presets.rs`, y `crates/lux/src/input/occurrence_tests.rs`. Verificado con `cargo run -p xtask`.

### ST05 — Plegado de Código Estilo Sublime (Code Folding) & Indicadores en Gutter
- Prioridad: `P0 (Core Sublime)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: Sistema completo de plegado de bloques de código en el gutter y por teclado.
- Alcance detallado:
  - **Indicadores en Gutter**:
    - Dibujar triángulos sutiles en el margen de números de línea: `▼` para bloques desplegados, `▶` para bloques plegados.
    - Clic sobre el triángulo conmuta el plegado del bloque.
  - **Marcador inline de bloque colapsado**:
    - Cuando un bloque está plegado, renderizar una píldora visual sutil `⋯` al final de la línea cabecera.
    - Clic en `⋯` despliega el bloque.
  - **Atajos de Teclado**:
    - `Fold` (`Cmd+Option+[` / `Ctrl+Shift+[`): pliega el bloque actual bajo el cursor.
    - `Unfold` (`Cmd+Option+]` / `Ctrl+Shift+]`): despliega el bloque actual.
    - `Fold All` (`Cmd+K, Cmd+1`): pliega todos los bloques de primer nivel en el archivo.
    - `Unfold All` (`Cmd+K, Cmd+J`): despliega todo el archivo.
    - `Fold by Level` (`Cmd+K, Cmd+2..5`): pliega por nivel de indentación o jerarquía sintáctica.
- Criterios de aceptación (Done when):
  - Hacer clic en la flecha del gutter pliega una función completa.
  - Las líneas plegadas no se renderizan en el viewport ni en el minimapa.
  - `Cmd+Option+[` y `Cmd+Option+]` pliegan y despliegan por teclado.
- Verificación:
  - Modelo de estado y mapeo visual de plegado implementado en `crates/lux-core/src/folding.rs`.
  - Conformado de líneas visuales en `crates/lux-ui/src/editor_view/mod.rs` y soporte en minimapa en `crates/lux-ui/src/minimap.rs`.
  - Geometría vectorial nítida de indicadores de gutter (`▼`/`▶`) y píldora inline `⋯` en `crates/lux/src/render/text.rs`.
  - Clic en gutter conmuta el pliegue en `crates/lux/src/chrome.rs` y clic en la píldora inline despliega en `crates/lux/src/input.rs`.
  - Navegación vertical ArrowUp/Down salta líneas ocultas en `crates/lux/src/input/motion.rs`.
  - Comandos en paleta (`code.fold`, `code.unfold`, `code.fold_all`, `code.unfold_all`, `code.fold_level_1..5`) en `crates/lux/src/palette/commands.rs`.
  - Entradas de menú nativo en `crates/lux/src/menu/mod.rs` y `crates/lux/src/menu/macos.rs`.
  - Atajos configurados en `crates/lux-config/keymaps/sublime.jsonc` y despacho en `crates/lux/src/input/dispatch.rs`.
  - 7 tests de integración en `crates/lux/src/input/folding_tests.rs`. Verificado con `cargo run -p xtask`.

### ST06 — Auto-Pairing, Auto-Closing de Llaves/Comillas y Bracket Matching Highlight
- Prioridad: `P0 (Core Sublime)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: Asistencia automática al escribir delimitadores y resaltado visual del par correspondiente.
- Alcance detallado:
  - **Auto-cierre inteligente**:
    - Escribir `(`, `{`, `[`, `"`, `'`, o `` ` `` inserta automáticamente el par de cierre correspondiente.
    - Si hay texto seleccionado, envolver la selección entre los delimitadores.
  - **Smart Step-Over**:
    - Si el cursor está justo antes del carácter de cierre (e.g. `)`) y el usuario escribe `)`, avanzar el cursor sobre él sin duplicar el carácter.
  - **Smart Backspace**:
    - Pulsar `Backspace` cuando el cursor está entre delimitadores vacíos (e.g. `(|)`) elimina ambos caracteres.
  - **Bracket Matching Highlight**:
    - Cuando el cursor está junto a un paréntesis, llave o corchete, dibujar un subrayado o recuadro sutil en el delimitador y en su pareja correspondiente.
    - Atajo `Control+M` (o `Cmd+Option+B`): saltar al delimitador emparejado.
- Criterios de aceptación (Done when):
  - Escribir `(` genera `()`.
  - Seleccionar `foo` y pulsar `"` produce `"foo"`.
  - Escribir `)` dentro de `(|)` resulta en `()|` sin duplicar.
  - El delimitador correspondiente se resalta visualmente en tiempo real.
- Verificación:
  - Auto-cierre de delimitadores `()`, `[]`, `{}`, `""`, `''`, y `` ` `` implementado en `crates/lux/src/input/autopair.rs`.
  - Envoltorio de selección (selection wrapping) para todos los pares.
  - Smart Step-Over sobre caracteres de cierre sin duplicación.
  - Smart Backspace eliminando pares vacíos atómicamente.
  - Detección de apóstrofo tras carácter alfanumérico para evitar cierres no deseados.
  - Resaltado visual de corchetes/paréntesis emparejados mediante `matching_brackets` en `crates/lux-ui/src/editor_view/mod.rs` y quads de subrayado dorado en `crates/lux/src/render/text.rs`.
  - Atajo `Control+M` para saltar al delimitador emparejado vía `jump_to_matching_bracket`, con entrada en menú nativo y comando en la paleta.
  - 7 tests de integración en `crates/lux/src/input/autopair_tests.rs`. Verificado con `cargo run -p xtask`.

---

## Track 2: Navegación "Goto Anything" Completa & Marcadores (P0)

### ST07 — Goto Anything Integral (`Cmd+P` con sintaxis `:line`, `@symbol`, `#term`)
- Prioridad: `P0 (Core Sublime)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: La experiencia unificada y ultra-rápida de Goto Anything de Sublime Text 3.
- Alcance detallado:
  - **Sintaxis unificada de prefijos y sufijos**:
    - Escribir texto plano: búsqueda difusa de archivos en todo el workspace (ordenado por relevancia y uso reciente).
    - Sufijo `:line` (e.g. `main.rs:120` o `:42`): salta inmediatamente a la línea indicada.
    - Sufijo `:line:col` (e.g. `main.rs:120:15`): salta a línea y columna específicas.
    - Prefijo o sufijo `@symbol` (e.g. `@build_frame` o `main.rs@build_frame`): salta al símbolo (función, método, struct) en el archivo usando Tree-sitter.
    - Prefijo o sufijo `#term` (e.g. `#render` o `main.rs#render`): salta a la coincidencia de texto dentro del archivo.
  - **Previsualización en Vivo (Transient Preview)**:
    - Al desplazarse por los resultados con flechas arriba/abajo, el editor previsualiza temporalmente el archivo y se desplaza a la línea exacta sin crear una pestaña permanente a menos que se pulse `Enter` o se edite.
- Criterios de aceptación (Done when):
  - Abrir `Cmd+P` y teclear `:50` salta en vivo a la línea 50 del buffer actual.
  - Teclear `@` lista todas las funciones y tipos del documento activo.
  - Teclear `app.rs:30` previsualiza `app.rs` en la línea 30; pulsar `Esc` regresa al archivo original sin modificar pestañas.
- Verificación:
  - Extracción de símbolos semánticos por Tree-sitter con clasificación exhaustiva (`fn`, `struct`, `class`, `trait`, `interface`, `enum`, `type`, `const`, `mod`, markdown `#`) y fallback en `crates/lux-syntax/src/symbols.rs`.
  - Sintaxis de consulta unificada (`Query`) con soporte para `:line`, `:line:col`, `@symbol`, `#term` y combinaciones `file:line`, `file@symbol`, `file#term` en `crates/lux-ui/src/goto_anything.rs`.
  - Módulo Goto desacoplado en `crates/lux/src/goto/mod.rs` y `crates/lux/src/goto/search.rs` manteniendo límites < 600 líneas por archivo.
  - Previsualización en vivo continua (`update_live_preview`) y captura de instantánea transitoria (`GotoSnapshot`) que restaura de forma fidedigna pestaña activa, cursores y scroll del viewport al presionar `Escape`.
  - Renderizado de badges temáticos de categoría de símbolo y números de línea alineados en `crates/lux/src/render/goto.rs`.
  - Atajos `Cmd+P`, `Cmd+R` (`@`), comandos en paleta `goto.line`, `goto.symbol`, `goto.word` y despacho en `crates/lux/src/input/dispatch.rs`.
  - 4 tests de integración completos en `crates/lux/src/input/goto_tests.rs`. Verificado con `cargo run -p xtask`.

### ST08 — Goto Symbol in Project (`Cmd+Shift+R`)
- Prioridad: `P0 (Core Sublime)`
- Estado: `DONE`
- Dependencias: ST07
- Objetivo: Búsqueda global instantánea de símbolos en todo el proyecto.
- Alcance detallado:
  - Modal flotante tipo Command Palette invocado con `Cmd+Shift+R` (macOS) / `Ctrl+Shift+R` (Win/Linux).
  - Consulta el índice de símbolos del proyecto generado asíncronamente en background vía `lux-workspace` y `lux-syntax`.
  - Muestra el nombre del símbolo, tipo (función, clase, constante, interfaz) y la ruta relativa del archivo.
  - Pulsar `Enter` abre el archivo en la posición exacta del símbolo.
- Criterios de aceptación (Done when):
  - Pulsar `Cmd+Shift+R` abre el modal inmediatamente.
  - Escribir el nombre de una función de otro archivo la localiza y salta a su definición al pulsar `Enter`.
- Verificación:
  - Extracción universal de símbolos basada en sintaxis o fallback en `crates/lux-syntax/src/symbols.rs` con función `extract_symbols_from_source`.
  - Indexador en background `SymbolIndex` y `SymbolIndexWorker` en `crates/lux-workspace/src/symbols.rs` con ranking por relevancia exacta, prefijo y subcadena, soporte para límites de tamaño de archivo (2 MB) y caché por mtime.
  - Sincronización en tiempo real de cambios de archivos y apertura de documentos en `crates/lux/src/session.rs` y `crates/lux/src/goto/search.rs`.
  - Modo `GotoMode::ProjectSymbols` en `crates/lux/src/goto/mod.rs` y renderizado de resultados con badges de categoría y rutas relativas con líneas en `crates/lux/src/render/goto.rs`.
  - Atajos `Cmd+Shift+R` / `Ctrl+Shift+R` configurados en `crates/lux-config/keymaps/sublime.jsonc`, atajo directo en `crates/lux/src/input.rs`, comando en paleta `goto.symbol_in_project` en `crates/lux/src/palette/commands.rs`, y menú nativo macOS en `crates/lux/src/menu/mod.rs` y `crates/lux/src/menu/macos.rs`.
  - 4 tests de integración específicos en `crates/lux/src/input/goto_tests.rs` (total 137 tests en `lux`, 56 en `lux-workspace`, 48 en `lux-syntax`). Verificado con `cargo run -p xtask`.

### ST09 — Sistema de Marcadores de Sublime (Bookmarks)
- Prioridad: `P1 (Sublime Parity)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: Sistema de marcadores en líneas con navegación cíclica y edición simultánea.
- Alcance detallado:
  - **Toggle Bookmark (`Cmd+F2` / `Ctrl+F2`)**:
    - Alterna un marcador en la línea actual.
    - Dibuja un icono visual sutil de marcador en el gutter junto al número de línea.
  - **Navegación de Marcadores**:
    - `Next Bookmark` (`F2`): salta cíclicamente al siguiente marcador del archivo.
    - `Previous Bookmark` (`Shift+F2`): salta al marcador anterior.
  - **Gestión masiva**:
    - `Clear Bookmarks` (`Cmd+Shift+F2` / `Ctrl+Shift+F2`): borra todos los marcadores del buffer.
    - `Select All Bookmarks` (`Option+F2` / `Alt+F2`): crea un cursor en cada línea marcada para edición multilínea inmediata.
- Criterios de aceptación (Done when):
  - `Cmd+F2` añade o quita un marcador visible en el gutter.
  - `F2` navega entre marcadores en orden secuencial.
  - `Option+F2` coloca cursores en todas las líneas marcadas simultáneamente.
- Verificación:
  - Modelo de estado desacoplado `BookmarkState` en `crates/lux-core/src/bookmarks.rs` con operaciones de alternancia (`toggle`, `toggle_at_cursors`), navegación cíclica (`next_bookmark`, `prev_bookmark`), limpieza (`clear`), edición simultánea (`select_all`) y ajuste por deltas de líneas (`adjust_for_line_delta`, `clamp_to_total_lines`). 7 tests unitarios en `lux-core`.
  - Persistencia y sincronización por pestaña en `DocumentEntry` y `App` en `crates/lux/src/document.rs`, `crates/lux/src/app.rs` y `crates/lux/src/app/bookmarks.rs`.
  - Icono vectorial en el margen izquierdo del gutter (cinta cian estilo Sublime Text) renderizado en `crates/lux/src/render/text.rs` e interacción de clic de ratón en el gutter en `crates/lux/src/input.rs`.
  - Atajos configurados en `crates/lux-config/keymaps/sublime.jsonc` (`ctrl+f2`/`cmd+f2`, `f2`, `shift+f2`, `ctrl+shift+f2`/`cmd+shift+f2`, `alt+f2`), despacho en `crates/lux/src/input/dispatch.rs`, comandos en paleta `bookmark.*` en `crates/lux/src/palette/commands.rs`, y submenú nativo de macOS "Bookmarks" en `crates/lux/src/menu/mod.rs` y `crates/lux/src/menu/macos.rs`.
  - 6 tests de integración en `crates/lux/src/input/bookmark_tests.rs` (total 143 tests en `lux`, todos pasando). Verificado con `cargo run -p xtask`.

---

## Track 3: Búsqueda y Reemplazo Estilo Sublime Text (P0/P1)

### ST10 — Barra de Búsqueda Flotante Inferior Docked (`Cmd+F`) & Reemplazo (`Cmd+Option+F` / `Cmd+H`)
- Prioridad: `P0 (Core Sublime)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: Barra inferior docked estilizada como Sublime Text con contador de coincidencias en vivo.
- Alcance detallado:
  - **Diseño Docked Inferior**:
    - Barra horizontal fijada al pie del editor con input estilizado.
    - Contador de coincidencias en tiempo real: `X of Y matches` (e.g. `1 of 14 matches`) o `No matches`.
  - **Resaltado en Viewport y Minimapa**:
    - Todas las coincidencias visibles se resaltan con un color suave de fondo o contorno.
    - Las marcas doradas de coincidencia (`[0.85, 0.65, 0.15, 0.70]` y activa `[1.0, 0.85, 0.20, 0.95]`) aparecen en la barra del minimapa.
  - **Botones de Opciones Interactivos**:
    - Regular Expression (`.*` / `Option+Cmd+R`).
    - Case Sensitive (`Aa` / `Option+Cmd+C`).
    - Whole Word (`"w"` / `Option+Cmd+W`).
    - In Selection (`Option+Cmd+S` / botón `"In Sel"`).
  - **Barra de Reemplazo (`Cmd+Option+F` / `Cmd+H`)**:
    - Fila adicional con input de reemplazo y botones interactivos: `Find`, `Find All`, `Replace`, `Replace All`.
  - **Navegación por Teclado**:
    - `Enter`: siguiente coincidencia. `Shift+Enter`: coincidencia anterior.
    - `Alt+Enter` en Find: `Find All` (convierte todas las coincidencias en un `MultiCursor` simultáneo).
    - `Alt+Enter` en Replace: `Replace All` (sustituye todas las coincidencias atómicamente).
    - `Esc`: cierra la barra y enfoca el editor en la coincidencia activa.
- Criterios de aceptación (Done when):
  - `Cmd+F` despliega la barra docked al pie del editor.
  - Escribir texto actualiza el contador de coincidencias instantáneamente.
  - Pulsar `Enter` cicla por las coincidencias con desplazamiento automático suave.
  - Pulsar `Esc` cierra la barra limpiamente.
- Verificación:
  - Motor de reemplazo atómico `replace_matches` en `crates/lux-core/src/find_replace.rs`.
  - Búsqueda acotada `set_query_scoped` y `replace_all` en `crates/lux-ui/src/search_bar.rs`.
  - Renderizado de barra docked con styling Sublime Text, botones e indicadores de minimapa en `crates/lux/src/render/search.rs` y `crates/lux/src/render/minimap.rs`.
  - Enrutamiento de atajos de teclado, focus trap y hit-testing de clicks en `crates/lux/src/search.rs`.
  - 10 tests de integración exhaustivos en `crates/lux/src/input/search_tests.rs` (total 153 tests en `lux`).
  - Verificado limpiamente con `cargo run -p xtask`.

### ST11 — Búsqueda en Archivos (Find in Files) con Buffer Virtual `Find Results` (`Cmd+Shift+F`)
- Prioridad: `P1 (Sublime Parity)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: Replicar el sistema de búsqueda en archivos de Sublime Text 3 con buffer interactivo editable `Find Results`.
- Alcance detallado:
  - **Panel Docked Inferior / Modal (`Cmd+Shift+F`)**:
    - Campos: `Find:`, `Where:` (e.g. `./src, *.rs, -*.test.rs`), `Replace:`.
  - **Buffer Virtual `Find Results`**:
    - Al ejecutar la búsqueda, abre una pestaña titulada `Find Results`.
    - Formato estándar de Sublime Text:
      ```text
      Searching 120 files for "fn update"
      
      /Users/.../crates/lux/src/app.rs:
        112: pub fn update(&mut self) {
        240:     self.update_views();
      
      2 matches across 1 file
      ```
  - **Navegación con doble clic o F4**:
    - Hacer doble clic en cualquier línea de resultado abre inmediatamente ese archivo en la línea y columna exactas.
    - Pulsar `F4` salta al siguiente resultado; `Shift+F4` salta al resultado anterior.
- Criterios de aceptación (Done when):
  - Ejecutar `Find in Files` abre la pestaña `Find Results` con resultados formateados.
  - Hacer doble clic en una línea salta al archivo correspondiente en la línea correcta.
  - `F4` / `Shift+F4` navegan secuencialmente entre los resultados.
- Verificación:
  - Motor de buffer virtual y navegación en `crates/lux/src/find_results.rs` con formateo idéntico a Sublime Text 3 (`Searching X files for "query"`, `Y matches across Z files`), parsing de líneas de resultado/cabeceras y saltos precisos.
  - Parser de cláusula `Where:` (`parse_where_query`) con soporte para rutas de inclusión y patrones de exclusión `-*.test.rs` / `!*.test.rs` en `crates/lux/src/project_search.rs`.
  - Detección de doble clic en `crates/lux/src/app.rs` y enter en `crates/lux/src/focus.rs` para saltar directamente al archivo y línea seleccionados.
  - Atajos `F4` (`next_result`) y `Shift+F4` (`prev_result`) configurados en `crates/lux-config/keymaps/sublime.jsonc`, despachados en `crates/lux/src/input/dispatch.rs`, registrados en la paleta de comandos (`find.next_result`, `find.prev_result`), y añadidos al menú nativo de macOS (`crates/lux/src/menu/mod.rs`, `crates/lux/src/menu/macos_builder.rs`).
  - 6 pruebas de integración dedicadas en `crates/lux/src/input/find_in_files_tests.rs`. Verificado limpiamente con `cargo run -p xtask`.

---

## Track 4: Layouts de Ventana, Grupos y Paneles Grid (P1)

### ST12 — Layouts Multi-Columna, Multi-Fila y Rejilla 2x2 (`Cmd+Option+1..5`)
- Prioridad: `P1 (Sublime Parity)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: Replicar la gestión de paneles y grupos de columnas y filas de Sublime Text 3.
- Alcance detallado:
  - **Atajos de Layout Estándar**:
    - `Single` (`Cmd+Option+1` / `Alt+Shift+1`): 1 solo panel (pantalla completa).
    - `2 Columns` (`Cmd+Option+2` / `Alt+Shift+2`): división vertical en 2 columnas iguales.
    - `3 Columns` (`Cmd+Option+3` / `Alt+Shift+3`): división vertical en 3 columnas iguales.
    - `4 Columns` (`Cmd+Option+4` / `Alt+Shift+4`): división vertical en 4 columnas.
    - `2 Rows` (`Cmd+Option+Shift+2` / `Alt+Shift+8`): división horizontal en 2 filas iguales.
    - `3 Rows` (`Cmd+Option+Shift+3` / `Alt+Shift+9`): división horizontal en 3 filas iguales.
    - `Grid 2x2` (`Cmd+Option+5` / `Alt+Shift+5`): rejilla de 4 paneles (2 columnas x 2 filas).
  - **Enfoque y Movimiento entre Grupos**:
    - `Focus Group 1..4` (`Ctrl+1..4`): enfoca directamente el panel número N.
    - `Move File to Group 1..4` (`Ctrl+Shift+1..4`): mueve la pestaña activa al grupo especificado.
  - **Divisores Arrastrables**:
    - Separadores interactivos entre paneles que permiten redimensionar el ancho o alto con el ratón.
- Criterios de aceptación (Done when):
  - Pulsar `Cmd+Option+2` divide la vista en 2 columnas.
  - Pulsar `Cmd+Option+5` configura una cuadrícula 2x2 de 4 paneles editables simultáneamente.
  - `Ctrl+2` enfoca la segunda columna; `Ctrl+Shift+2` mueve la pestaña actual a esa columna.
  - Arrastrar los divisores con el ratón redimensiona proporcionalmente los paneles.
  - Hacer clic en cualquier panel transfiere el foco activo a dicho grupo.
- Verificación:
  - Implementado en `crates/lux/src/split.rs`, `crates/lux/src/render/split.rs`, `crates/lux/src/chrome.rs`, `crates/lux/src/app.rs`, `crates/lux/src/input/dispatch.rs`, `crates/lux/src/palette/registry.rs`, `crates/lux/src/palette/commands.rs`, `crates/lux/src/menu/macos_view_menus.rs` y `crates/lux-config/keymaps/sublime.jsonc`.
  - 6 pruebas de integración dedicadas en `crates/lux/src/input/layout_tests.rs` y tests unitarios en `crates/lux/src/split.rs`. Verificado limpiamente con `cargo run -p xtask`.

---

## Track 5: Sistema de Compilación (Sublime Build Systems) & Consola (P1)

### ST13 — Sublime Build System Runner (`Cmd+B`, `Cmd+Shift+B`)
- Prioridad: `P1 (Sublime Parity)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: Ejecución de builds del proyecto y salto interactivo a errores de compilación (`F4`).
- Alcance detallado:
  - **Build Runner (`Cmd+B` / `Ctrl+B`)**:
    - Detecta automáticamente el sistema de construcción según el proyecto abierto:
      - Cargo: `cargo check` / `cargo build` / `cargo test` / `cargo clippy` / `cargo run`
      - Python: `python3 <archivo>` / `pytest`
      - Node/npm: `npm test` o scripts en `package.json` (`npm run build`)
      - Go: `go build` / `go test` / `go run <archivo>`
      - C/C++: `make` / `make clean`
      - Rust (standalone): `rustc <archivo>`
      - Bash: `bash <archivo>`
  - **Build With... (`Cmd+Shift+B` / `Ctrl+Shift+B`)**:
    - Despliega un menú difuso para seleccionar la variante de build deseada (e.g. `Cargo: Test`, `Cargo: Release`, `Cargo: Clippy`, `Python: Run File`).
  - **Panel Inferior Docked `Build Output`**:
    - Muestra la salida del proceso en streaming sin bloquear la UI a través del subsistema PTY/tareas.
    - Atajo `Tools -> Cancel Build` o `Cmd+Option+C` / `Ctrl+Break` para detener y terminar procesos en ejecución.
  - **Navegación de Errores con F4 (`file_regex`)**:
    - Parsea errores de compilador con regex (`ruta:línea:col`) y enlaces detectados por PTY (`detect_links`).
    - Pulsar `F4` salta automáticamente al siguiente archivo y línea con error.
    - Pulsar `Shift+F4` salta al error anterior.
- Criterios de aceptación (Done when):
  - Pulsar `Cmd+B` en un proyecto Rust ejecuta `cargo check` y muestra la salida en el dock inferior.
  - Si hay un error de sintaxis, pulsar `F4` abre el archivo y posiciona el cursor en la línea del error.
  - `Cmd+Shift+B` permite elegir entre diferentes variantes de compilación.
- Verificación:
  - Implementado en `crates/lux/src/build.rs`, `crates/lux/src/render/build.rs`, `crates/lux/src/render/modals.rs`, `crates/lux/src/input/dispatch.rs`, `crates/lux/src/ui.rs`, `crates/lux/src/palette/registry.rs`, `crates/lux/src/palette/commands.rs`, `crates/lux/src/menu/mod.rs` y `crates/lux/src/menu/macos_view_menus.rs`.
  - 6 integration tests dedicados en `crates/lux/src/input/build_tests.rs`. Verificado limpiamente con `cargo test --workspace` (174 tests en `lux`) y `cargo run -p xtask`.

### ST14 — Soporte de Archivos de Proyecto Sublime (`.sublime-project`)
- Prioridad: `P1 (Sublime Parity)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: Carga y guardado de proyectos compatibles con la especificación de Sublime Text.
- Alcance detallado:
  - Formato JSON con soporte para claves estándar:
    - `folders`: lista de carpetas con `path`, `folder_exclude_patterns`, `file_exclude_patterns`, `name`.
    - `settings`: anulaciones de configuración específicas para ese proyecto (e.g. `tab_size`, `rulers`).
    - `build_systems`: comandos de construcción personalizados asociados al proyecto que alimentan el Build Runner (`Cmd+B` / `Cmd+Shift+B`).
  - Comandos:
    - `Project: Open Project...`
    - `Project: Save Project As...`
    - `Project: Quick Switch Project` (`Cmd+Ctrl+P` / `Ctrl+Alt+P`): lista difusa para alternar entre proyectos recientes al instante.
    - `Project: Close Project`
  - Descubrimiento automático de proyectos `.sublime-project` en la raíz del workspace y apertura directa desde la CLI (`lux my_app.sublime-project`).
- Criterios de aceptación (Done when):
  - Abrir un archivo `.sublime-project` carga sus carpetas, exclusiones y ajustes en el workspace.
  - `Cmd+Ctrl+P` permite alternar rápidamente entre proyectos sin reiniciar el editor.
- Verificación:
  - Implementado en `crates/lux-workspace/src/project/mod.rs`, `crates/lux-workspace/src/project/tests.rs`, `crates/lux/src/project.rs`, `crates/lux/src/render/project_picker.rs`, `crates/lux/src/render/modals.rs`, `crates/lux/src/input/dispatch.rs`, `crates/lux/src/palette/registry.rs`, `crates/lux/src/palette/commands.rs`, `crates/lux/src/menu/mod.rs` y `crates/lux/src/menu/macos_view_menus.rs`.
  - Integración en `crates/lux/src/build.rs` alimentando las variantes de compilación de `Cmd+B` / `Cmd+Shift+B`.
  - 6 integration tests en `crates/lux/src/input/project_tests.rs` y tests unitarios en `crates/lux-workspace/src/project/tests.rs`. Verificado con `cargo test --workspace` (180 tests en `lux`, 57 en `lux-workspace`) y `cargo run -p xtask`.

---

## Track 6: Temas Icónicos de Sublime & Modo Distraction Free (P1/P2)

### ST15 — Esquemas de Color Icónicos de Sublime Text (Monokai, Mariana, Breakers, Celeste)
- Prioridad: `P1 (Sublime Parity)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: Incluir de fábrica los temas de color legendarios de Sublime Text 3 calibrados con exactitud.
- Alcance detallado:
  - **Monokai**: El tema oscuro clásico por excelencia de Sublime Text 3 (fondo `#272822`, palabras clave rosa `#F92672`, cadenas amarillo `#E6DB74`, funciones verde `#A6E22E`, números/constantes morado `#AE81FF`).
  - **Mariana**: El esquema moderno por defecto de Sublime 3.1+ (tonos pizarra azulados `#303841`, acentos cian, naranja y menta).
  - **Breakers**: Tema oscuro de alto contraste y tonalidad oceánica (`#0C1014`, acento cian `#00D0D0`, verde `#5AF78E`, rosa `#FF6B8B`).
  - **Celeste**: Tema claro pulido y descansado para la vista (`#F8F9FA`, acento `#007ACC`, morado `#A626A4`, azul `#4078F2`).
  - **Selector en Vivo**:
    - Comando `Preferences: Color Theme` con vista previa en vivo en el buffer conforme se navega con flechas arriba y abajo.
    - Modal flotante con badges de previsualización de colores en tiempo real, confirmación con `Enter` y restauración instantánea con `Escape`.
- Criterios de aceptación (Done when):
  - Seleccionar `Monokai` aplica exactamente los colores de sintaxis y fondo de Sublime Text 3.
  - Cambiar de tema en la paleta actualiza la ventana en tiempo real sin recargar la aplicación.
- Verificación:
  - Implementado en `crates/lux-ui/src/theme.rs` (`Theme::MONOKAI`, `Theme::MARIANA`, `Theme::BREAKERS`, `Theme::CELESTE`, `by_name`), `crates/lux-syntax/src/theme/mod.rs` y `crates/lux-syntax/src/theme/presets.rs`.
  - Selector difuso en `crates/lux/src/theme_picker.rs` y renderizado modal con swatches en `crates/lux/src/render/theme_picker.rs` y `crates/lux/src/render/modals.rs`.
  - Comando `Preferences: Color Theme` y `UI: Select Color Scheme` en `crates/lux/src/palette/registry.rs` y `crates/lux/src/palette/commands.rs`, despacho en `crates/lux/src/input/dispatch.rs`, y menú nativo macOS en `crates/lux/src/menu/mod.rs` y `macos_builder.rs`.
  - 6 integration tests dedicados en `crates/lux/src/input/theme_tests.rs` (186 tests en `lux`), y tests unitarios en `lux-ui` y `lux-syntax`. Verificado con `cargo test --workspace` y `cargo run -p xtask`.

### ST16 — Modo Libre de Distracciones (Distraction Free Mode - `Shift+F11`)
- Prioridad: `P1 (Sublime Parity)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: Espacio de escritura puro a pantalla completa idéntico a Sublime Text 3.
- Alcance detallado:
  - **Activación (`Shift+F11` / `Cmd+Ctrl+Shift+F`)**:
    - Pone la ventana a pantalla completa mediante `PlatformWindow::set_fullscreen(true)`.
    - Oculta la barra de pestañas (`tab_bar_height = 0`), barra de estado (`status_bar_height = 0`), sidebar (`sidebar_width = 0`), minimapa (`minimap_width = 0`) y gutter de números de línea (`show_gutter = false`).
    - Centra el texto del documento en una columna de ancho fijo (configurable vía `editor.distractionFreeColumnWidth`, por defecto 80 caracteres) con márgenes calculados dinámicamente.
  - **Salida instantánea**:
    - Pulsar `Shift+F11`, `Cmd+Ctrl+Shift+F` o `Esc` (cuando no hay selecciones, modales ni búsquedas activas) restaura exactamente el estado visual previo (sidebar, minimapa, pantalla completa).
  - **Comandos y Menús**:
    - Comando de paleta `View: Toggle Distraction Free Mode` (`view.toggle_distraction_free`).
    - Menú nativo macOS `View -> Enter Distraction Free Mode` / `Exit Distraction Free Mode`.
- Criterios de aceptación (Done when):
  - Pulsar `Shift+F11` oculta todos los cromos de la interfaz y centra el texto en pantalla completa.
  - Pulsar `Shift+F11` o `Escape` de nuevo restaura exactamente el estado visual previo.
- Verificación:
  - Implementado en `crates/lux-platform/src/window.rs` y `crates/lux-platform/src/desktop/window.rs` (`set_fullscreen`, `is_fullscreen`).
  - Lógica central en `crates/lux/src/distraction_free.rs` (`DistractionFreeSnapshot`, `DistractionFreeState`, `toggle_distraction_free`, `enter_distraction_free`, `exit_distraction_free`, `text_left`), `crates/lux/src/ui.rs` y `crates/lux/src/app/views.rs`.
  - Renderizado sin cromos y centrado en `crates/lux/src/render.rs` y `crates/lux/src/render/text.rs`.
  - Atajos, despacho y manejo de `Escape` en `crates/lux/src/input.rs`, `crates/lux/src/input/dispatch.rs` y `crates/lux-config/keymaps/sublime.jsonc`.
  - Esquema de configuración `editor.distractionFreeColumnWidth` en `crates/lux-config/src/schema.rs` y `crates/lux/src/settings.rs`.
  - Paleta de comandos en `crates/lux/src/palette/registry.rs` y `commands.rs`, y menú nativo macOS en `crates/lux/src/menu/mod.rs`, `macos_view_menus.rs` y `macos.rs`.
  - 6 integration tests dedicados en `crates/lux/src/input/distraction_free_tests.rs`. Verificado limpiamente con `cargo test --workspace` (192 tests en `lux`), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` y `cargo run -p xtask`.

### ST17 — Limpieza Automática de Espacios en Blanco y Nueva Línea al Guardar
- Prioridad: `P2 (Ergonomía)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: Comportamiento de guardado limpio estándar de Sublime Text.
- Alcance detallado:
  - `trim_trailing_white_space_on_save`:
    - Al guardar (`Cmd+S`), elimina automáticamente los espacios y tabuladores redundantes al final de cada línea modificada (`editor.trimTrailingWhitespace`).
    - Opción de proteger la línea en la que se encuentra el cursor activo para no interferir con la edición en curso (`editor.trimTrailingWhitespacePreserveCursor`).
  - `ensure_newline_at_eof_on_save`:
    - Garantiza que los archivos terminen siempre con un carácter de nueva línea (`\n`, `editor.insertFinalNewline`).
  - Comandos manuales en Command Palette:
    - `File: Trim Trailing Whitespace` (`file.trim_trailing_whitespace`).
    - `File: Ensure Final Newline` (`file.ensure_final_newline`).
- Criterios de aceptación (Done when):
  - Guardar un archivo con espacios al final de línea los elimina limpiamente.
  - El cambio se registra como parte del estado limpio del archivo en disco.
- Verificación:
  - Funciones puras en `crates/lux-core/src/edit_ops/whitespace.rs` (`trim_trailing_whitespace`, `ensure_final_newline`) y `crates/lux-core/src/edit_ops/mod.rs`.
  - Transformaciones on-save y comandos en `crates/lux/src/document/save_ops.rs` (`apply_on_save_transforms`, `trim_trailing_whitespace`, `ensure_final_newline`, `finalize_whitespace_edits`) integrados en el despacho de guardado de `crates/lux/src/document.rs`.
  - Esquema y ajustes en `crates/lux-config/src/schema.rs` (`editor.trimTrailingWhitespace`, `editor.trimTrailingWhitespacePreserveCursor`, `editor.insertFinalNewline`) y `crates/lux/src/settings.rs`.
  - Paleta de comandos en `crates/lux/src/palette/registry.rs`, `commands.rs` y `crates/lux/src/input/dispatch.rs`.
  - 9 unit tests en `lux-core` y 6 integration tests dedicados en `crates/lux/src/document/whitespace_tests.rs`. Verificado con `cargo test --workspace` (198 tests en `lux`), `cargo clippy --workspace --all-targets -- -D warnings`, `check-limits.sh` y `cargo run -p xtask`.

---

## Track 7: Snippets de Sublime Text & Macros (P2)

### ST18 — Motor de Snippets Extensible de Sublime Text
- Prioridad: `P2 (Sublime Parity)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: Expansión de snippets con trigger + Tab y soporte de tabstops interactivos.
- Alcance detallado:
  - Formato de snippet Sublime (`.sublime-snippet` / JSON):
    - `tabTrigger`: palabra clave que dispara el snippet al pulsar `Tab` (e.g. `lorem`, `fn`, `struct`).
    - `scope`: lenguajes a los que aplica (e.g. `source.rust`, `source.python`).
    - `content`: plantilla con `$1`, `${2:default_value}`, `$0`.
  - Navegación interactiva con `Tab` (siguiente tabstop) y `Shift+Tab` (tabstop previo).
  - Soporte de espejos (mirrors): un identificador que se repite en varias partes del snippet se actualiza en tiempo real mientras se escribe.
- Criterios de aceptación (Done when):
  - Escribir `fn` y pulsar `Tab` en un archivo Rust inserta `fn nombre() {\n\t\n}` con el cursor en `nombre`.
  - Pulsar `Tab` salta al cuerpo de la función `$0`.
- Verificación:
  - Parseadores de snippets XML Sublime (`.sublime-snippet` con CDATA, `<tabTrigger>`, `<scope>`, `<description>`) y diccionarios/listas JSON en `crates/lux/src/snippets/parser.rs`.
  - Biblioteca completa de snippets integrados para Rust, Python, JavaScript, TypeScript, HTML, Go, C, C++, Markdown, CSS y Lorem Ipsum en `crates/lux/src/snippets/builtins.rs`.
  - Registro de snippets `SnippetRegistry` con carga recursiva desde `~/.config/lux/snippets/`, `~/.lux/snippets/` y `<workspace>/.lux/snippets/`, y resolución contextual de scopes con normalización de alias de lenguaje en `crates/lux/src/snippets/registry.rs`.
  - Sesión interactiva de snippets en `crates/lux-core/src/snippets/mod.rs` con navegación bidireccional (`Tab` / `Shift+Tab`), reemplazo de placeholders al escribir, sincronización en tiempo real de espejos idénticos (mirrors) para inserción de texto y Backspace atómico, y cancelación con `Escape`.
  - Modal interactivo de selección rápida `SnippetPickerState` con búsqueda difusa, previsualización de triggers/descripciones y badges en `crates/lux/src/snippets/picker.rs`, renderizado en `crates/lux/src/render/snippet_picker.rs` y `crates/lux/src/render/modals.rs`.
  - Atajos `Cmd+Alt+S` / `Ctrl+Alt+S`, comandos de paleta (`snippet.insert`, `snippet.next_tabstop`, `snippet.prev_tabstop`), y despacho automático al pulsar `Tab` sobre triggers válidos en `crates/lux/src/snippets/dispatch.rs` y `crates/lux/src/input.rs`.
  - 22 tests unitarios en `lux-core` y 8 tests de integración dedicados en `crates/lux/src/input/snippet_tests.rs`. Verificado con `cargo test --workspace` (206 tests en `lux`), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` y `cargo run -p xtask`.

### ST19 — Grabación y Reproducción de Macros de Teclado
- Prioridad: `P2 (Sublime Parity)`
- Estado: `DONE`
- Dependencias: —
- Objetivo: Automatización rápida de patrones repetitivos mediante macros de teclado.
- Alcance detallado:
  - **Record Macro (`Ctrl+Q`)**:
    - Inicia o detiene la grabación de comandos, movimientos y pulsaciones de teclas.
    - Muestra un indicador en la barra de estado: `● Recording Macro`.
  - **Playback Macro (`Cmd+Shift+Q` / `Ctrl+Shift+Q`)**:
    - Reproduce la secuencia grabada en la posición del cursor actual (o en todos los cursores si hay multi-cursor activo).
  - **Save Macro As...**:
    - Opción de guardar la macro en `~/.lux/macros/<nombre>.sublime-macro` para asignarle un atajo de teclado en la configuración.
- Criterios de aceptación (Done when):
  - Pulsar `Ctrl+Q`, realizar una transformación en una línea, y pulsar `Ctrl+Q` graba la macro.
  - Pulsar `Cmd+Shift+Q` en otra línea replica la transformación exacta.
- Verificación:
  - Definición de comandos de macro y serialización bidireccional JSON compatible con Sublime Text 3 (`.sublime-macro` con `{"command": "...", "args": {...}}`) en `crates/lux/src/macros/command.rs` y `crates/lux/src/macros/macro_def.rs`.
  - Estado de macros `MacroState` con descubrimiento recursivo y carga automática de macros desde `~/.lux/macros/`, `~/.config/lux/macros/` y `<workspace>/.lux/macros/` en `crates/lux/src/macros/recording.rs`.
  - Motor de ejecución y reproducción `playback_last_macro` / `execute_macro` en `crates/lux/src/macros/playback.rs` con soporte completo para multi-cursor simultáneo, inserción de texto, saltos de línea, supresión de caracteres/palabras, movimientos de cursor y comandos de edición (`duplicate_line`, `toggle_comment`, etc.).
  - Ganchos de captura en tiempo real en el bucle de eventos en `crates/lux/src/macros/dispatch.rs` e `input.rs` registrando texto, saltos de línea, backspace, delete, borrado de palabras y movimientos sin auto-grabación recursiva durante la reproducción.
  - Indicador visual `● Recording Macro` en color coral/rojo vivo en la barra de estado interactiva en `crates/lux/src/render/status_bar.rs` y `render.rs`.
  - Atajos de teclado en `crates/lux-config/keymaps/sublime.jsonc` (`Ctrl+Q` para alternar grabación, `Cmd+Shift+Q` / `Ctrl+Shift+Q` para reproducir, y `Cmd+Alt+Shift+S` / `Ctrl+Alt+Shift+S` para guardar), comandos registrados en la Command Palette (`macro.toggle_record`, `macro.playback`, `macro.save_as`), y acciones del menú nativo (`MenuAction::ToggleRecordMacro`, `PlaybackMacro`, `SaveMacro`).
  - 6 integration tests dedicados en `crates/lux/src/input/macro_tests.rs`. Verificado con `cargo test --workspace` (214 tests en `lux`), `cargo clippy --workspace --all-targets -- -D warnings`, `check-limits.sh` y `cargo run -p xtask`.

---

## Non-Goals Permanentes (Regla #7 de CLAUDE.md)

Para mantener la velocidad instantánea (< 100ms de inicio), ligereza de memoria (< 50MB RSS) y fiabilidad extrema de Lux, **jamás se implementará**:
- **Cero Debugger / DAP**: No se añadirán puntos de interrupción, inspección de variables ni marcos de pila.
- **Cero IA / LLM Integrado**: No habrá chat lateral, modelos de predicción ni extensiones tipo Copilot en el núcleo.
- **Cero Telemetría**: Cero rastreo, análisis de uso o subida de reportes a la red.
- **Cero Colaboración en Tiempo Real / Nube**: No habrá WebSockets, CRDTs de red, cuentas de usuario ni login remoto.
- **Cero Tienda Remota de Plugins**: Únicamente plugins locales basados en WASM Component Model.

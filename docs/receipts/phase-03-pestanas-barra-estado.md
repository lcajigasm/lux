# Receipt: Pestañas + barra de estado (entregable 9)

- Phase: 03
- Crate(s): lux-ui
- Goal: El entregable "Pestanas + barra de estado" de la Fase 3 ("Sintaxis,
  navegación y paleta") de `LUX_PROMPT_MAESTRO.md` está completo y su gate
  pasa sin ningún fallo ni test ignorado.
- Verify: `cargo test -p lux-ui tabs:: status_bar::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Alcance.** El texto completo del entregable 9: "pestañas con drag &
  drop, pinning, cierre, reapertura de la última cerrada; estado con
  posición del cursor, lenguaje, codificación, finales de línea e
  indentación, todos clicables." Como en el resto de la Fase 3, no existe
  ninguna UI interactiva real en `lux-ui` todavía (ni renderizado de
  pestañas, ni manejo de eventos de ratón/arrastre) — así que, igual que
  los cuatro receipts anteriores de esta fase, esto es la capa de estado
  que una UI real consumiría, no la UI en sí. `docs/STATUS.md` mantiene la
  fila "Tabs + status bar" en `not-started` (regla 1) con la prosa
  detallando lo construido.

  **`tabs.rs`: `TabBar`.** `open`/`close`/`reopen_last_closed`
  (pila LIFO — cerrar A y luego B y reabrir dos veces devuelve B primero,
  luego A)/`pin`/`unpin`/`move_tab`/`activate`. Dos decisiones de diseño
  explícitas:
  - **Ids acuñados por el propio `TabBar`, nunca elegidos por quien
    llama** (`TabId(u64)` opaco, `next_id` interno). A diferencia de
    `command_palette::Command` (cuyo id sí debe ser estable y elegido
    fuera, porque futuros keybindings lo referenciarán), una pestaña no
    tiene ese requisito, así que un id auto-generado elimina la clase
    entera de "dos pestañas con el mismo id" en vez de necesitar un
    `Result` de error para detectarla — más simple que el registro de
    comandos, a propósito, porque el problema que resuelve es distinto.
  - **`pin`/`unpin` reordenan activamente** (mueven la pestaña al final
    del grupo de fijadas / al inicio del grupo de no fijadas) en vez de
    limitarse a poner un booleano — fijar sin agrupamiento visual sería
    una función que no hace nada útil. `move_tab`, en cambio, no impone
    esa agrupación: es el primitivo crudo "mover a la posición X" que un
    gesto de arrastre reporta, y la mayoría de editores permiten
    interleaving manual libre durante un arrastre.
  - `close` reactiva un vecino sensato (el de la derecha, o el de la
    izquierda si se cerró la última) en vez de dejar `active` colgando o
    saltar arbitrariamente a la primera pestaña.

  **`status_bar.rs`: reutiliza detección de Fase 1, no la reinventa.**
  `lux-core::buffer` ya expone `eol()`/`indent()`/`encoding()` detectados
  (fila "EOL / indent / encoding detection" de `docs/STATUS.md`,
  `implemented` desde la Fase 1). Este módulo sólo los formatea para
  mostrar ("LF"/"CRLF"/"CR", "UTF-8"/"UTF-8 BOM"/"UTF-16 LE"/"UTF-16
  BE"/"Latin-1", "Tabs"/"Spaces: N") con `match` exhaustivos sin rama
  comodín — si `lux-core` añade una variante nueva a `Eol`/`Encoding`/
  `IndentStyle`, esto deja de compilar en vez de mostrar una etiqueta
  incorrecta en silencio. `language` es un `&str` que pasa quien llama
  (no hay detección de lenguaje conectada a `lux-ui` todavía; eso es
  `lux-syntax`, fuera de este entregable). La posición de cursor se
  formatea 1-based ("Ln 2, Col 1"), la convención de Sublime/VS Code, a
  partir del `Position` 0-based de `lux-core`.

  **"Todos clicables" resuelto igual que "todo comando de menú está en la
  paleta" del entregable anterior: por estructura, no por convención.**
  `StatusBarItem` no tiene ningún constructor que permita crear un item
  sin `command_id` — no existe la variante "sólo etiqueta". Y
  `status_bar::register_commands` registra esos mismos cinco ids
  (`status_bar.goto_line`, `.change_language`, `.change_encoding`,
  `.change_eol`, `.change_indentation`) en un `command_palette::
  CommandRegistry` real. El test
  `every_built_status_bar_item_resolves_to_a_registered_command` verifica
  end-to-end que cada `command_id` que produce `build()` aparece en el
  registro tras `register_commands` — un click siempre resuelve a un
  comando real, nunca a un string suelto sin backing. Esto conecta este
  entregable con el de la paleta de comandos (mismo crate, mismo
  registro) en vez de inventar un segundo mecanismo de "acción" paralelo.

  **Gate con `--`.** `cargo test -p lux-ui tabs:: status_bar::` tal cual
  falla — `cargo test` sólo acepta un filtro suelto; dos filtros
  separados por espacio necesitan `--` para llegar al harness. Mismo caso
  ya documentado en
  `docs/receipts/phase-03-bracket-pair-colorization-folding-real.md`. El
  comando que de verdad se ejecutó: `cargo test -p lux-ui -- tabs::
  status_bar::`.

  **Verificación real.** `cargo test -p lux-ui -- tabs:: status_bar::`:
  25/25 passed, 0 ignorados (15 tests de `tabs::`: abrir/activar, cerrar
  con reactivación de vecino derecho/izquierdo/ninguno, cerrar no cambia
  el activo si no era el cerrado, cerrar id desconocido, reabrir en orden
  LIFO, reabrir sin nada cerrado, pin agrupa al inicio, pin dos veces es
  no-op estable, unpin mueve al inicio del grupo no fijado, move_tab
  reordena por id, move_tab satura fuera de rango al final, mover/activar
  id desconocido devuelven `false`; 10 tests de `status_bar::`: cada
  variante de `Eol`/`Encoding`, formato de indentación tabs/espacios,
  cursor 1-based, EOL/encoding reales del buffer, etiqueta de lenguaje
  provista por quien llama, registro de los cinco comandos, registro
  duplicado falla, y la integración build+registry). Gate completo del
  repo en verde: `cargo run -p xtask` (fmt sin diffs desde el primer
  intento; clippy `-D warnings` en el workspace entero sin hallazgos;
  `cargo test --workspace`: 235 tests en `lux-core` + el resto del
  workspace sin regresiones, 83 en total en `lux-ui` (58 previos + 25
  nuevos); `cargo deny check`; límites de fichero — `tabs.rs` 326 líneas,
  `status_bar.rs` 210 líneas, ambos lejos del límite de 600; aislamiento
  de `lux-core`). Ningún struct nuevo pasa de 5 campos (`StatusBar`). No
  se añadió ninguna dependencia nueva. Verificado sólo en macOS, misma
  limitación documentada en el resto de `docs/STATUS.md`.

  **Qué queda fuera de este entregable** (resto de la Fase 3, sin tocar):
  la UI real de pestañas y barra de estado (renderizado, manejo de
  eventos de ratón/arrastre — no existe ninguna UI de interacción en
  `lux-ui` todavía, mismo estado que dejaron los cuatro receipts
  anteriores de esta fase), los comandos reales detrás de
  `status_bar.change_language`/`.change_encoding`/`.change_eol`/
  `.change_indentation`/`.goto_line` (registrados pero sin implementación
  — son acciones futuras, no de este entregable), minimapa. Se dejó
  intacto, sin tocar ni comitear, el trabajo pendiente de entregables
  previos que ya estaba en el árbol al empezar este loop (auto-indent en
  `lux-syntax`, y Goto Anything / paleta de comandos / barra de búsqueda
  de los tres loops anteriores, cada uno con su propio receipt, aún sin
  commitear).

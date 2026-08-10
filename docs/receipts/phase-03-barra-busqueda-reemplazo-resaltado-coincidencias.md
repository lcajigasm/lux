# Receipt: Barra de búsqueda/reemplazo + resaltado de coincidencias (entregable 8)

- Phase: 03
- Crate(s): lux-ui
- Goal: El entregable "Barra de busqueda/reemplazo + resaltado de
  coincidencias" de la Fase 3 ("Sintaxis, navegación y paleta") de
  `LUX_PROMPT_MAESTRO.md` está completo y su gate pasa sin ningún fallo ni
  test ignorado.
- Verify: `cargo test -p lux-ui search_bar::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Alcance real de este entregable.** El texto completo es "Barra de
  búsqueda y reemplazo en el buffer, con regex, y navegación entre
  resultados con resaltado de todas las coincidencias". Antes de escribir
  nada se revisó `crates/lux-core/src/find_replace.rs` — y resulta que el
  motor entero ya existe y está completo desde la Fase 1: `Query`
  (literal/case-insensitive/whole-word/regex, las cuatro vía un único
  `regex::Regex` compilado), `find_all`, `find_next`, `replace_one`,
  `replace_all`, todo transaccional/deshacible. `docs/STATUS.md` ya
  marcaba "Find/replace in buffer" como `implemented` en la sección de
  Fase 1. Lo que no existía era la parte de UI: qué resultado está
  "activo" ahora mismo, y mantener eso sincronizado según cambia el
  buffer. Ese es el trabajo real de este loop —
  `crates/lux-ui/src/search_bar.rs`, un módulo nuevo, cero lógica de
  matching nueva.

  **Por qué navegar por índice y no reusar `find_next`.** `lux-core`
  también expone `find_next(buffer, query, after: Position)`, que busca el
  siguiente match a partir de una posición, con wraparound — pero está
  pensado para "saltar al resultado más cercano al cursor", no para una
  barra que ya necesita la lista completa de coincidencias en memoria para
  resaltarlas todas. Ya que esa lista existe siempre (`SearchBar::matches`),
  navegar es sólo incrementar/decrementar un índice con módulo — sin
  volver a ejecutar la regex en cada `next_match()`/`previous_match()`, y
  sin necesitar convertir posiciones. `find_next` queda intacto y sigue
  siendo la función correcta para otros casos (p. ej. un futuro "buscar
  desde el cursor" en la Fase de wiring de UI real).

  **`next`/`previous` se llaman `next_match`/`previous_match`.** Primer
  intento con los nombres cortos; clippy (`should_implement_trait`) los
  rechazó porque `fn next(&mut self) -> Option<T>` se confunde con
  `Iterator::next` — nombre más específico, sin implementar `Iterator`
  (no tendría sentido: la "iteración" aquí tiene estado de navegación
  bidireccional con wraparound, no es un consumo lineal de una sola
  pasada).

  **Regex inválido a medio escribir no borra los resultados.** `set_query`
  llama a `find_all` con la query nueva *antes* de mutar `self`; si falla
  (regex incompleto, p. ej. escribiendo `"foo("` letra a letra), el error
  se propaga y el estado anterior (última query válida + sus matches) se
  queda tal cual, en vez de vaciarse. Mismo criterio que ya se usó para el
  sufijo `:línea` de Goto Anything: un estado transitorio inválido
  mientras el usuario teclea no debe tirar el último resultado bueno.

  **`replace_current` avanza "gratis".** Al reemplazar la coincidencia en
  el índice `i` y recalcular `matches` contra el buffer ya mutado, esa
  ocurrencia desaparece de la lista y todo lo posterior se desplaza un
  índice hacia atrás — dejar `current` sin tocar tras el refresh hace que
  apunte, sin ningún caso especial, a lo que antes era la *siguiente*
  coincidencia. Verificado explícitamente en
  `replace_current_replaces_and_advances_to_what_was_the_next_match`.

  **Verificación real.** `cargo test -p lux-ui search_bar::`: 11/11
  passed, 0 ignorados (cálculo de matches + arranque en el primero, sin
  matches, opciones regex/case-insensitive combinadas, regex inválido
  preserva estado previo, wraparound en ambas direcciones, sin matches
  devuelve `None`, `refresh` recoge ediciones hechas fuera de la barra,
  reemplazo único con avance automático, reemplazo único sin coincidencia
  activa es no-op, reemplazo único deshacible vía la `Transaction`
  devuelta, reemplazo global deja la lista vacía). Un fallo real durante
  el desarrollo: dos aserciones de test con un rango mal calculado a mano
  (`17..20` en vez de `16..19` para la tercera "cat" de
  `"cat bat cat hat cat"`) — error de aritmética en el test, no del
  código; corregido tras contar los índices con cuidado. Gate completo del
  repo en verde: `cargo run -p xtask` (fmt — aplicado tras escribir el
  fichero; clippy `-D warnings` en el workspace entero — el único hallazgo
  real fue el de `next`/`previous` de arriba; `cargo test --workspace`:
  235 tests en `lux-core` + el resto del workspace sin regresiones —
  incluye los 26 tests de `find_replace::` reverificados intactos; `cargo
  deny check`; límites de fichero — `search_bar.rs` en 283 líneas, lejos
  del límite de 600; aislamiento de `lux-core`). `SearchBar` tiene 3
  campos. No se añadió ninguna dependencia nueva: todo sobre lo que ya
  exponía `lux-core` (`buffer`, `find_replace`, `history`, `selection`).
  Verificado sólo en macOS, misma limitación documentada en el resto de
  `docs/STATUS.md`.

  **Qué queda fuera de este entregable** (resto de la Fase 3, sin tocar):
  la UI real de la barra (`Cmd+F`/`Cmd+H`, diálogo, keybindings — Fase 5),
  el paso de resaltado real en `EditorView` (no existe ninguna UI de
  interacción en `lux-ui` todavía, mismo estado que dejaron los dos
  receipts anteriores de esta fase), pestañas/barra de estado, minimapa.
  Se dejó intacto, sin tocar ni comitear, el trabajo pendiente de
  entregables previos que ya estaba en el árbol al empezar este loop
  (auto-indent en `lux-syntax`, y Goto Anything / paleta de comandos de
  los dos loops anteriores, cada uno con su propio receipt, aún sin
  commitear).

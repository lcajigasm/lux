# Receipt: Paleta de comandos unificada con menús (entregable 7)

- Phase: 03
- Crate(s): lux-search, lux-ui
- Goal: El entregable "Paleta de comandos unificada con menus" de la Fase 3
  ("Sintaxis, navegación y paleta") de `LUX_PROMPT_MAESTRO.md` está
  completo y su gate pasa sin ningún fallo ni test ignorado.
- Verify: `cargo test -p lux-ui command_palette::`
- Iterations: 1 / 10
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Alcance.** El entregable 7 completo dice: "registro único de comandos
  con categorías, ranking por uso reciente y *context keys* (`when`
  clauses) para habilitación contextual. Todo comando de menú debe estar
  en la paleta". No hay todavía ninguna UI interactiva en `lux-ui` (sólo
  widgets pasivos, mismo estado que dejó el receipt de Goto Anything), así
  que no existe ningún menú real que "unificar". Por eso, igual que con
  Goto Anything, `docs/STATUS.md` mantiene la fila "Command palette" en
  `not-started` (regla 1) y documenta en prosa lo que sí existe: el
  registro único, el ranking fuzzy + recencia, y el parser de `when`
  clauses — la parte de "unificación con menús" se resuelve por
  construcción (ver más abajo), no con una UI que todavía no toca
  construirse.

  **Cómo se resuelve "todo comando de menú debe estar en la paleta" sin
  que exista ningún menú.** La lectura literal de "unificado con menús"
  cuando no hay menús sería imposible de verificar. La lectura que sí es
  verificable ahora: hacer estructuralmente imposible que exista una lista
  de comandos de menú separada del registro. `CommandRegistry` es el único
  sitio de todo `lux-ui` donde se puede definir un comando — no hay ningún
  tipo "ítem de menú" en el crate. Cuando exista un menú real, no tendrá
  otra fuente de la que sacar sus ítems más que
  `CommandRegistry::commands()`, el mismo registro que la paleta busca.
  Eso es justamente lo que v0 hizo mal (dos listas mantenidas a mano que
  divergieron) y lo que esto hace imposible por diseño, no por disciplina.
  El test `no_separate_menu_list_every_registered_command_is_in_the_palette`
  documenta esta propiedad: todo lo registrado aparece en una búsqueda sin
  filtrar, porque no hay ninguna otra vía de registro.

  **Motor de ranking: se reutiliza `nucleo`, no se reimplementa.**
  `docs/adr/0009-search-and-fuzzy-matching.md` ya asigna `nucleo` a
  `lux-search` para "Goto Anything and the command palette" — literalmente
  este entregable. `lux-search::fuzzy::rank_paths` (del loop anterior) no
  servía tal cual: usa `Config::match_paths()` (bonus por `/`) y opera
  sobre `&str` puros, pero los comandos son structs y dos comandos pueden
  compartir título (p. ej. "Format Document" en dos categorías). Se añadió
  `rank_by`, genérica sobre cualquier `T` vía una función `key: Fn(&T) ->
  &str`, con `Config` por defecto (sin el sesgo de rutas, que no tiene
  sentido para títulos de comando) — sin duplicar la lógica de scoring de
  `nucleo`, sólo el punto de entrada. `rank_paths` queda intacta.

  **Ranking por uso reciente: reloj lógico, no reloj de pared.** En vez de
  `SystemTime`/`Instant`, `CommandRegistry` lleva un contador `u64` que se
  incrementa en cada `record_use`; sólo importa el orden relativo entre
  comandos, nunca el tiempo real transcurrido, así que un contador
  monótono es estrictamente suficiente y hace los tests deterministas sin
  ningún `sleep` ni mock de reloj. Con query vacío, el orden es
  puramente por recencia (usados primero, nunca-usados al final en orden
  de registro). Con query no vacío, el criterio primario es la puntuación
  fuzzy y la recencia sólo desempata puntuaciones iguales — para que un
  comando viejo con mejor coincidencia nunca quede enterrado bajo uno
  reciente con peor coincidencia.

  **`when` clauses: parser real, no un `if` con un único caso.** Se
  implementó un mini-lenguaje booleano de verdad —`&&`/`||`/`!`/`(...)`
  con la precedencia estándar (`!` > `&&` > `||`)— con tokenizador y
  parser recursivo-descendente propios (~90 líneas), sobre un `Context` de
  claves booleanas (ausente = `false`, igual que el `when` de VS Code). Se
  consideró limitar el alcance a "una sola clave, sin operadores" (más
  ponytail), pero eso no es lo que dice el entregable ("*context keys*
  (`when` clauses)") ni lo que hace falta de verdad: una regla de
  habilitación contextual real casi siempre necesita negar o combinar más
  de una condición (p. ej. `editorFocus && !hasSelection`), así que el
  paréntesis/precedencia no es alcance especulativo, es el caso de uso
  mínimo. Una cláusula vacía (`""`) es siempre verdadera (comando sin
  restricción); una cláusula malformada (paréntesis sin cerrar, operador
  colgante, carácter desconocido) es un `Err(WhenParseError)`, nunca se
  trata en silencio como "siempre verdadera" ni como "siempre falsa" —
  ambas opciones esconderían un error de configuración real.

  **`register` puede fallar.** Un id de comando duplicado devuelve
  `Err(DuplicateCommandId)` en vez de sobreescribir en silencio o entrar
  en pánico — es un error de programación plausible (dos features
  registrando el mismo id) sobre la única invariante estructural del
  registro (ids únicos), así que merece manejo explícito, a diferencia de
  casos que de verdad no pueden ocurrir.

  **Verificación real.** `cargo test -p lux-ui command_palette::`: 12/12
  passed, 0 ignorados (registro duplicado, `when` con clave suelta,
  negación/`&&`/`||`, precedencia `&&` sobre `||`, paréntesis
  sobreescribiendo precedencia, cláusula vacía siempre verdadera,
  cláusulas malformadas rechazadas, `search` filtrando por `when`, ranking
  fuzzy por título, orden por recencia con query vacío, desempate por
  recencia con query igual, y la invariante de "un único registro").
  `cargo test -p lux-search fuzzy::`: 8/8 passed (los 6 tests previos de
  `rank_paths` intactos más 2 nuevos de `rank_by`). Gate completo del
  repo en verde: `cargo run -p xtask` (fmt — hubo que reformatear tras
  escribir el fichero, aplicado con `cargo fmt`; clippy `-D warnings` en
  el workspace entero — se corrigió un `useless_conversion` real
  (`.into_iter()` innecesario al pasar un `Vec` a un parámetro
  `impl IntoIterator`); `cargo test --workspace`: 235 tests en `lux-core` +
  el resto del workspace sin regresiones; `cargo deny check`; límites de
  fichero — `command_palette.rs` queda en 547 líneas, dentro del límite de
  600 pero el más cercano al límite del proyecto hasta ahora, anotado aquí
  para quien añada `@symbol`/`#text`/historial a Goto Anything o extienda
  esta paleta, por si hace falta partir el fichero entonces; aislamiento
  de `lux-core`). Ningún struct nuevo pasa de 4 campos
  (`CommandRegistry`). Verificado sólo en macOS, misma limitación
  documentada en el resto de `docs/STATUS.md`.

  **Qué queda fuera de este entregable** (resto de la Fase 3, sin tocar):
  la UI real de `Cmd+Shift+P` (no existe ninguna UI de interacción en
  `lux-ui` todavía), el menú real cuyos ítems consumirían el registro,
  keybindings (Fase 5, `lux-config`), Goto Anything `@símbolo`/`#texto`/
  historial/previsualización (entregable anterior, ya cerrado con su
  propio receipt), barra de búsqueda/reemplazo, pestañas/barra de estado,
  minimapa. Se dejó intacto, sin tocar ni comitear, el trabajo pendiente
  de entregables previos que ya estaba en el árbol al empezar este loop
  (`crates/lux-syntax/src/lib.rs` modificado y
  `crates/lux-syntax/src/indent_rules.rs` sin seguimiento, de auto-indent;
  y los cambios de Goto Anything del loop anterior, ya con su propio
  receipt, tampoco comiteados).

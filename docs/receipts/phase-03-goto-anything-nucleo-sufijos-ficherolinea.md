# Receipt: Goto Anything (núcleo + sufijos fichero:línea)

- Phase: 03
- Crate(s): lux-search, lux-ui
- Goal: El entregable "Goto Anything (nucleo + sufijos fichero:linea)" de
  la Fase 3 ("Sintaxis, navegación y paleta") de `LUX_PROMPT_MAESTRO.md`
  está completo y su gate pasa sin ningún fallo ni test ignorado.
- Verify: `cargo test -p lux-ui goto_anything::`
- Iterations: 1 / 10
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Alcance.** El entregable 6 completo de la Fase 3 en
  `LUX_PROMPT_MAESTRO.md` agrupa en una sola frase varias capacidades:
  fuzzy con `nucleo`, sufijos `fichero:línea`/`@símbolo`/`#texto`,
  historial de recientes y previsualización. El objetivo de este loop lo
  acota explícitamente a "núcleo + sufijos fichero:línea": el motor de
  ranking fuzzy respaldado por `nucleo`, más el parseo del sufijo
  `:línea` estilo Sublime. `@símbolo`, `#texto`, historial y
  previsualización quedan fuera, igual que la UI real (`Cmd+P`, diálogo,
  keybinding) y el índice real de ficheros del proyecto (eso es
  `lux-workspace`, Fase 4, todavía no construido). Por eso
  `docs/STATUS.md` mantiene la fila "Goto Anything" en `not-started`
  (regla 1: una función existe entera o no existe) y documenta el avance
  interno en prosa en vez de marcar la fila como completa.

  **Dónde vive el motor de matching.**
  `docs/adr/0009-search-and-fuzzy-matching.md` ya decidía, antes de este
  loop, que `nucleo` vive en `lux-search` ("`lux-search` embeds ... `nucleo`
  for fuzzy matching in Goto Anything and the command palette") — no en
  `lux-ui`. Se siguió esa decisión: `lux-search::fuzzy::rank_paths` es la
  única función nueva ahí, un envoltorio fino sobre
  `nucleo::pattern::Pattern::match_list` con
  `Config::DEFAULT.match_paths()` (el preset de `nucleo` pensado para
  rutas: `/` puntúa como límite de segmento). `lux-ui::goto_anything` sólo
  conoce el parseo del sufijo y compone el resultado; no reimplementa
  nada del matching. `lux-ui` ya podía depender de `lux-search` según
  `docs/ARCHITECTURE.md` (crate de dominio, no UI/render).

  **Por qué `Pattern::match_list` y no el `Nucleo` de alto nivel.** El
  crate `nucleo` expone dos capas: `Nucleo<T>`, pensado para *pickers*
  interactivos (hilos de fondo, `tick()`, snapshots) — necesario cuando
  hay que rankear cientos de miles de ficheros sin bloquear un hilo de
  UI, pero no algo que este loop deba montar (no hay UI ni índice real
  de proyecto todavía); y `Pattern::match_list`, la función síncrona de
  una sola hebra que la propia documentación de `nucleo` describe como
  la vía recomendada para listas pequeñas y que da resultados
  deterministas, sin hilos ni temporizadores, ideal para tests. El
  reparto entre los dos vive documentado en el propio ADR 0009 y en el
  encabezado de `fuzzy.rs`: cuando el índice real de Fase 4 exista y el
  volumen lo justifique, migrar a `Nucleo<T>` es un cambio local a
  `lux-search`, no a la sintaxis del sufijo en `lux-ui`.

  **Parseo del sufijo `fichero:línea`.** `Query::parse` busca el último
  `:` con `rsplit_once`; sólo lo trata como sufijo de línea si lo que
  sigue analiza como un entero positivo (`line @ 1..`, 1-based como
  Sublime). Cualquier otra cosa — sin dos puntos, `:0`, `:abc`, o un `:`
  final sin dígitos todavía (el estado transitorio mientras el usuario
  sigue escribiendo) — se deja como texto literal de búsqueda en vez de
  perderse o fallar, para que teclear progresivamente nunca descarte lo
  ya escrito.

  **Tests, no sólo cobertura.** El ADR 0009 pide explícitamente "tests
  de caracterización" del comportamiento de ranking de `nucleo`, para
  que un fork o un reemplazo propio sea barato si hace falta — por eso
  `lux-search::fuzzy` tiene sus propios tests (bonus de límite de ruta,
  sensibilidad a mayúsculas `Smart`, hueco entre caracteres, query
  vacío) además de los que exige el gate de `lux-ui`. `goto_anything::`
  en `lux-ui` cubre el parseo del sufijo (texto plano, sufijo válido,
  `:0` rechazado, sufijo no numérico, `:` final sin dígitos, sufijo sin
  parte de fichero) y la composición completa (`search`): línea
  adjuntada a cada resultado, candidatos no coincidentes excluidos,
  query vacío tras el sufijo devuelve todos los candidatos sin
  puntuar (caso "saltar a la línea N del fichero actual").

  **Verificación real.** `cargo test -p lux-ui goto_anything::`: 10/10
  passed, 0 ignorados. `cargo test -p lux-search fuzzy::`: 6/6 passed.
  Gate completo del repo en verde: `cargo run -p xtask` (fmt, clippy
  `-D warnings` en el workspace entero — se corrigió un
  `collapsible_if` real detectado por clippy en `Query::parse`,
  `cargo test --workspace`: 235 tests en `lux-core` + el resto del
  workspace sin regresiones, `cargo deny check` — la licencia MPL-2.0 de
  `nucleo`/`nucleo-matcher` pasa el check de licencias, sin duplicados
  nuevos —, límites de fichero, aislamiento de `lux-core`).
  `crates/lux-search/src/fuzzy.rs` (75 líneas) y
  `crates/lux-ui/src/goto_anything.rs` (195 líneas) muy lejos del límite
  de 600; ningún struct nuevo pasa de 3 campos. Verificado sólo en
  macOS, misma limitación documentada en el resto de `docs/STATUS.md`.

  **Qué queda fuera de este entregable** (resto de la Fase 3, sin
  tocar): sufijos `@símbolo` y `#texto`, historial de recientes,
  previsualización, la UI real de `Cmd+P` (no existe ninguna UI de
  interacción en `lux-ui` todavía, sólo widgets pasivos), el índice real
  de ficheros del proyecto (`lux-workspace`, Fase 4), paleta de
  comandos, barra de búsqueda/reemplazo, pestañas/barra de estado,
  minimapa. Se dejó intacto, sin tocar ni comitear, el trabajo no
  relacionado que ya estaba pendiente en el árbol de trabajo al empezar
  este loop (`crates/lux-syntax/src/lib.rs` modificado,
  `crates/lux-syntax/src/indent_rules.rs` y el receipt de auto-indent sin
  seguimiento) — es el entregable anterior, ya resuelto y con su propio
  receipt, sólo pendiente de commit por quien lo cierre.

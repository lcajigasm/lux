# Receipt: Cierre de Fase 3 — presupuestos de rendimiento y huecos de Goto Anything

- Phase: 03
- Crate(s): lux-syntax, lux-search (indirecto), lux-ui, benches
- Goal: Los bloqueos identificados al intentar cerrar la Fase 3 con
  `/close-phase 3` — los dos presupuestos de rendimiento de la fase sin
  medir, el criterio de los 16 ms de resaltado sin verificar, y los
  sub-sufijos pendientes de Goto Anything — quedan resueltos: medidos con
  datos reales donde son medibles, corregidos donde había un problema real
  de arquitectura, e implementados o formalmente diferidos vía ADR donde
  procede.
- Verify: `cargo bench -p lux-benches --bench phase3_budgets` (presupuestos);
  `cargo test -p lux-syntax` (nuevo `highlight_range` + `highlight`
  intacto); `cargo test -p lux-ui goto_anything::` (historial +
  previsualización); `cargo run -p xtask` (gate completo del repo)
- Iterations: 1 / — (no era un loop de un entregable único; ver notas)
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Contexto.** Al ejecutar `/close-phase 3`, el paso 2 (comprobar
  receipts) técnicamente pasaba — los 9 receipts de la fase dicen
  `Result: passed` — pero contrastarlo contra los propios criterios de
  aceptación de la fase y contra `docs/STATUS.md` reveló brechas reales:
  dos de los cuatro criterios de aceptación nunca se habían medido, un
  tercero era literalmente imposible de verificar tal como estaba el
  código, y el entregable 6 (Goto Anything) tenía sub-sufijos pendientes
  sin resolver. Se paró ahí en vez de cerrar la fase simulando un estado
  que `docs/STATUS.md` contradecía. Este loop resuelve esos bloqueos.

  **1. Presupuestos de rendimiento medidos por primera vez.**
  `benches/benches/phase3_budgets.rs` (nuevo, en el crate `lux-benches`,
  mismo patrón que `phase1_budgets.rs`): genera 10k/20k líneas de Rust
  real y parseable, construye un `tree_sitter::InputEdit` correcto a mano
  (nunca se había ejercitado `Document::edit` en ningún test de
  `lux-syntax` hasta ahora — ni siquiera en `golden_tokens.rs`) y mide
  `Document::edit` en aislamiento para el presupuesto de reparseo. Para
  Goto Anything, genera 100.000 rutas sintéticas y mide
  `lux_search::fuzzy::rank_paths` directamente.

  **Resultados reales** (ver `docs/PERFORMANCE.md` para la tabla completa
  con fecha y contexto): reparseo tras pulsación en 10k líneas, **1.05 ms**
  contra presupuesto de 2 ms — pasa. Goto Anything sobre 100k ficheros,
  **1.34 ms** contra presupuesto de 150 ms — pasa con más de 100× de
  margen (con la salvedad documentada de que "primeros resultados < 30 ms"
  no es una fase medible por separado en una implementación síncrona; ver
  más abajo).

  **2. El criterio de los 16 ms reveló un problema real de arquitectura,
  no solo faltaba medirlo.** La primera medición de reparseo+resaltado
  tras una pulsación en un fichero de 20k líneas dio **~24.6 ms** — muy
  por encima del presupuesto. Investigado, no aceptado como "está bien,
  ya se arreglará": `Document::highlight` recorre el árbol **entero** en
  cada llamada (`cursor.captures(&query, tree.root_node(), ...)`, sin
  acotar rango), así que cada pulsación de tecla re-resaltaría el
  documento completo, no solo lo visible — exactamente lo que la
  virtualización de `EditorView` (Fase 2, entregable 4) existe para
  evitar en el shaping de glifos, pero que el resaltado nunca había
  heredado.

  **Corrección real, no un ajuste del benchmark.** Se añadió
  `Document::highlight_range(source, byte_range)` en
  `crates/lux-syntax/src/lib.rs`, usando
  `tree_sitter::QueryCursor::set_byte_range` (acota de verdad qué
  recorre el cursor, no es un filtro posterior sobre resultados ya
  calculados). `highlight()` ahora delega en
  `highlight_range(source, 0..source.len())` — verificado byte-idéntico
  contra los 18 tests de `golden_tokens.rs`, que siguen intactos sin
  modificar ni un solo fichero de fixture. Un segundo hallazgo apareció
  al medir la versión acotada a 50 líneas (~11.6 ms — mejor, pero
  sorprendentemente alto para solo 50 líneas): `Query::new` — compilar el
  patrón de resaltado completo desde el `.scm`, cientos de líneas — se
  estaba recompilando en **cada** llamada a `highlight`/`highlight_range`,
  y resultó ser el coste dominante, no el recorrido del árbol. Se cacheó
  como campo de `Document` (`highlight_query: Query`, construida una sola
  vez en `Document::new`). Efecto combinado de ambas correcciones:
  documento completo, de ~26 ms a ~16.8 ms (con caché de query pero sin
  acotar rango — sigue rozando/superando el presupuesto, mantenido a
  propósito en el benchmark como prueba de por qué acotar el rango es
  necesario, no una alternativa); rango visible de 50 líneas, de ~11.6 ms
  a **~2.3 ms** — el número real que debe cumplir el presupuesto, con
  amplio margen.

  **3. `@símbolo` y `#texto`: diferidos con ADR, no simplemente
  ausentes.** Antes de implementarlos se comprobó qué hace falta de
  verdad para cada uno: `@símbolo` necesita un índice real de símbolos
  (LSP `workspace/symbol`, Fase 7, o consultas de outline por gramática
  en `lux-syntax`, que no existen y serían alcance nuevo real); `#texto`
  necesita búsqueda de texto completo en todo el proyecto, que es
  literalmente el entregable 4 de la Fase 4 ("Find in Files con el motor
  de ripgrep embebido"). Construirlos ahora significaría o bien
  infraestructura desechable que Fase 4/7 reemplazará, o adelantar esas
  fases por iniciativa propia — la regla 7 ("Scope is closed... no
  extiendas el roadmap por tu cuenta") lo prohíbe explícitamente. Se
  documentó la decisión en
  `docs/adr/0014-goto-anything-symbol-text-suffixes-deferred.md`, con
  qué desbloquea cada sufijo y por qué, para que quien retome esto
  cuando Fase 4/7 existan no tenga que re-derivar el razonamiento.

  **4. Historial de recientes y previsualización, sí implementados —
  no dependen de ninguna fase futura.** `RecentFiles` en
  `crates/lux-ui/src/goto_anything.rs`: mismo reloj lógico que
  `command_palette::CommandRegistry` (sin marca de tiempo real, solo
  orden relativo, determinista y testeable sin `sleep`). `search_with_recency`:
  query vacío → lista por recencia (los nunca abiertos al final, en su
  orden original); query no vacío → puntuación fuzzy primero, recencia
  solo desempata — un fuzzy match claramente mejor nunca pierde contra uno
  peor solo por haberse abierto hace más tiempo (verificado explícitamente
  en un test, tras un primer intento con datos de prueba donde la
  intuición sobre el scoring de `nucleo` resultó equivocada — ver más
  abajo). `preview(buffer, line, radius_lines)`: un snippet de texto
  alrededor de la línea objetivo, a partir de un `Buffer` ya cargado por
  quien llama — deliberadamente **no** abre el fichero él mismo, porque
  eso sería I/O en lo que eventualmente sería el hilo de UI (regla de oro
  #6); leer el fichero sigue siendo responsabilidad de quien llame,
  igual que cada otro módulo de este crate.

  **Un fallo real durante el desarrollo.** El primer test de "la
  recencia no debe ganarle a un fuzzy match claramente mejor" usaba
  `"b/mainframe_unrelated_padding.rs"` como el candidato peor — falló: el
  scoring real de `nucleo` lo puntuó más alto de lo esperado (el bonus de
  límite de ruta tras `/` más el match contiguo de "main" al principio de
  "mainframe" compite más de cerca de lo intuido con "a/main.rs"). No se
  ajustó el umbral ni se debilitó la aserción — se cambió el candidato a
  uno donde "main" solo encaja con huecos amplios
  (`"b/manual_index.rs"`), una diferencia de calidad de match inequívoca,
  y el test pasó por la razón correcta.

  **Verificación real.** `cargo bench -p lux-benches --bench
  phase3_budgets`: los cuatro números de arriba, reproducidos tras cada
  cambio de código (no solo una vez). `cargo test -p lux-syntax`: 44 tests
  unitarios + 18 `golden_tokens` = 62, todos en verde, incluyendo 3 tests
  nuevos de `highlight_range` (delega igual que `highlight` sobre el
  documento entero, excluye tokens fuera de rango, vacío antes del primer
  parseo). `cargo test -p lux-ui goto_anything::`: 17/17 (10 previos +
  7 nuevos de `RecentFiles`/`search_with_recency`/`preview`). Gate
  completo del repo en verde: `cargo run -p xtask` (fmt, clippy
  `-D warnings` en el workspace entero sin hallazgos tras los ajustes de
  formato; `cargo test --workspace`: 235 en `lux-core` + el resto del
  workspace sin regresiones, 102 en `lux-ui` (95 previos + 7 netos
  nuevos, con 10 sustituidos por 17), 44+18 en `lux-syntax`; `cargo deny
  check` — sin duplicados nuevos; límites de fichero —
  `crates/lux-syntax/src/lib.rs` 376 líneas, `crates/lux-ui/src/
  goto_anything.rs` 356 líneas, `benches/benches/phase3_budgets.rs` 246
  líneas, todos lejos del límite de 600; aislamiento de `lux-core`).
  `Document` pasa de 3 a 4 campos (añade `highlight_query`), bien lejos
  del límite de 25.

  **Qué sigue sin resolver, a propósito.** El hueco estructural real que
  queda tras este loop no es de rendimiento ni de alcance de Goto
  Anything: **ningún entregable de la Fase 3 está conectado a una UI
  real todavía** — ni un solo token resaltado pinta en pantalla, no hay
  diálogo de `Cmd+P` ni de `Cmd+Shift+P`, ninguna pestaña se renderiza.
  Es el mismo estado que documentaba cada receipt anterior de esta fase,
  y arreglarlo no es una limpieza de cierre de fase — es construir el
  wiring de interacción real para cinco subsistemas distintos, con su
  propio alcance y sus propias decisiones de diseño. Se deja fuera de
  este loop deliberadamente, y se reporta como la decisión pendiente real
  antes de etiquetar `v0.2.0` (ver el mensaje de cierre de esta sesión).

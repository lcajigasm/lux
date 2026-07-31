# Receipt: Bracket pair colorization + folding real (entregables 3 y 4)

- Phase: 03
- Crate(s): lux-syntax
- Goal: El entregable "Bracket pair colorization + folding real" de la Fase
  3 ("Sintaxis, navegación y paleta") está completo y su gate pasa sin
  ningún fallo ni test ignorado.
- Verify: `cargo test -p lux-syntax -- folding:: brackets::`
- Iterations: 3 / 10
- Result: passed
- Opened: 2026-07-31
- Closed: 2026-07-31
- Notes:

  **Sobre el comando de gate literal.** El texto del entregable dice
  `cargo test -p lux-syntax folding:: brackets::` (sin `--`). Probado tal
  cual, falla: `error: unexpected argument 'brackets::' found` — `cargo
  test` solo acepta un filtro posicional directo; para pasar varios
  filtros (que el harness de test sí soporta, unidos por OR) hace falta el
  separador `--`. El comando real y equivalente que ejecuté es `cargo test
  -p lux-syntax -- folding:: brackets::`, que sí corre ambos módulos. Es
  una errata trivial de sintaxis, no una ambigüedad de alcance como la del
  primer entregable — la intención (correr los tests de `folding::` y
  `brackets::`) es inequívoca, así que no paré a pedir aclaración; lo dejo
  documentado aquí y en `docs/STATUS.md` para que quede registrado.

  **Diseño — brackets.** `crates/lux-syntax/src/brackets.rs`:
  `bracket_pairs(&Document)` camina cada hoja del árbol real (no un escaneo
  de texto) con una pila por tipo de paréntesis/corchete/llave, emparejando
  aperturas con cierres del mismo tipo y calculando la profundidad de
  anidamiento en el momento del emparejamiento. `pair_at(pairs, byte_offset)`
  encuentra "el par bajo el cursor" (el segundo requisito del entregable:
  resaltar ambos delimitadores). El color por profundidad vive en
  `theme::SyntaxTheme` (nuevo campo `bracket_colors`, una paleta cíclica de
  6 colores por tema, igual que la convención de VS Code) en vez de un
  sistema de color aparte — reutiliza la infraestructura del entregable
  anterior en vez de duplicarla.

  **Hallazgo real durante el trabajo**: la primera versión fallaba el test
  `an_unbalanced_opener_contributes_no_pair` (`"[1, 2"` sin cerrar en JSON)
  porque emparejaba un corchete que nunca se escribió. Causa: el árbol de
  recuperación de errores de tree-sitter sintetiza un nodo `MISSING` de
  anchura cero con el `kind()` correcto (`"]"`) para mantener el árbol bien
  formado tras un error de sintaxis; mi código lo trataba como un cierre
  real. Arreglado comprobando `!node.is_missing()` antes de considerar un
  nodo como paréntesis/corchete/llave real.

  **Diseño — folding.** `crates/lux-syntax/src/folding.rs`:
  `folding_ranges(&Document)` recorre el mismo árbol y marca como plegable
  cualquier nodo cuyo `end_position().row > start_position().row` —
  derivado enteramente de los spans reales del árbol, nunca de contar
  `{`/`[`/indentación (la heurística de v0 que
  `LUX_PROMPT_MAESTRO.md` nombra explícitamente como el error a no repetir
  en este entregable). Como muchos nodos anidados empiezan en la misma fila
  (`fn foo() {` inicia tanto el `function_item` como su `block`), se
  deduplica quedándose con el rango más ancho por fila de inicio. El nodo
  raíz (el fichero entero) se excluye a propósito — plegar "todo" no es una
  funcionalidad útil.

  **Verificación real.** `cargo test -p lux-syntax -- folding:: brackets::`:
  13/13 passed, 0 ignorados. Los tests no son triviales: comprueban
  anidamiento real de profundidad, que tipos de paréntesis distintos nunca
  se emparejan entre sí, que un paréntesis dentro de un string no cuenta
  como bracket real, que un plegado no aparece para una línea única aunque
  tenga llaves, y que el árbol —no el conteo de llaves— es la fuente de
  verdad incluso cuando hay una llave dentro de un string en la misma
  línea. Gate completo del repo en verde: `cargo run -p xtask` (fmt,
  clippy `-D warnings` en el workspace entero — encontró y corregí un
  `collapsible_if` real usando un let-chain, no un `#[allow]` — `cargo test
  --workspace`, `cargo deny check`, límites de fichero, aislamiento de
  `lux-core`). Los dos entregables anteriores de esta fase (`highlight::`,
  `theme::`) se re-verificaron intactos en la misma pasada: 23 tests de
  librería + 18 golden tokens, todos en verde. Ningún fichero de la Fase 3
  supera 600 líneas; `SyntaxTheme` pasó de 4 a 5 campos (`bracket_colors`),
  bien lejos del límite de 25. No se añadió ninguna dependencia nueva.

  **Qué queda fuera de este entregable** (resto de la Fase 3, sin tocar):
  auto-indent por lenguaje, Goto Anything, paleta de comandos, barra de
  búsqueda/reemplazo, pestañas/barra de estado, minimapa — y, dentro del
  propio alcance de brackets/folding, wiring hacia el render real de
  `lux-ui` (pintar los colores de bracket y los marcadores de plegado en el
  gutter), que es trabajo posterior, no de esta librería. Verificado solo
  en macOS, misma limitación documentada en el resto de `docs/STATUS.md`.

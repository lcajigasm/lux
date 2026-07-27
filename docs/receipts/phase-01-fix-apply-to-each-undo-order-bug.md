# Receipt: Fix — `apply_to_each` invertía el orden de `Transaction.edits`

- Phase: 01
- Crate(s): lux-core
- Goal: Corregir el bug, señalado en el receipt de "buscar/reemplazar", por
  el que `edit_ops::apply_to_each` dejaba `Transaction.edits` en el orden
  equivocado para deshacer una edición multicursor.
- Verify: `cargo test -p lux-core edit_ops::tests::apply_to_each`
- Iterations: 1 / —
- Result: passed
- Opened: 2026-07-27
- Closed: 2026-07-27
- Notes:

  **Causa raíz confirmada, idéntica a la de `find_replace::replace_all`**:
  `all_edits` se construye con `.extend(edits)` dentro de un bucle que
  procesa los cursores de atrás hacia delante (posición más alta primero) —
  ese orden de recorrido, combinado con que cada `op(buffer, sel)` ya aplica
  sus propias ediciones al buffer en su propio orden interno antes de
  devolverlas, significa que `all_edits` YA queda en el orden cronológico
  real en que se aplicaron al buffer, sin necesitar ningún ajuste. Había un
  `all_edits.reverse()` de más al final que lo revolvía. Se eliminó esa
  única línea.

  **Importante**: `selections_after.reverse()`, que está justo al lado en
  el código, NO es el mismo bug y se dejó intacta — esa sí hace falta, por
  una razón distinta: `raw_after`/`shift_at_capture` se construyen en el
  mismo orden descendente del bucle (cursor de posición más alta primero),
  pero `selections_before` viene de `multicursor.cursors()`, que está
  siempre ordenado ascendente (invariante de `MultiCursor`). Para que
  `selections_before[i]` y `selections_after[i]` sigan correspondiéndose al
  mismo cursor en `Transaction::new`, `selections_after` sí necesita
  revertirse a orden ascendente. Confundir estas dos reversiones — una
  necesaria, una no — habría sido fácil; se dejó un comentario explícito en
  el código señalando la diferencia para que no se repita.

  **Verificación real, no solo confianza en el diagnóstico anterior**: se
  añadieron dos tests de regresión que no existían antes y que sí habrían
  fallado con el código viejo:
  - `apply_to_each_transaction_is_undoable_and_redoable` — llama
    `transaction.undo()` y `transaction.apply()` directamente y compara el
    texto del buffer contra el original y contra el estado tras aplicar.
  - `apply_to_each_transaction_undoes_correctly_through_history` — el mismo
    escenario pero pasando por un `History` real (`record` + `undo`),
    porque `History::undo` también depende de que `Transaction.edits` esté
    en el orden correcto al revertirlo.

  Ambos tests fallaban antes del fix (confirmado revirtiendo el cambio
  momentáneamente y volviendo a correrlos) y pasan después. Los dos tests
  ya existentes de `apply_to_each` (que solo comprobaban `buffer.text()` y
  las posiciones de cursor resultantes, no el undo) seguían pasando incluso
  con el bug — es exactamente por qué el bug pasó desapercibido durante
  todo un entregable previo.

  **Para promover a memoria**: cualquier función que construya una
  `Transaction` necesita al menos un test que llame a `.undo()` (o pase por
  `History`) sobre su resultado, no solo verificar el estado final del
  buffer — ya es la segunda vez en esta fase que el mismo tipo de bug
  (un `.reverse()` de más, con el estado final del buffer accidentalmente
  correcto de todos modos) se cuela exactamente por esa falta de cobertura.

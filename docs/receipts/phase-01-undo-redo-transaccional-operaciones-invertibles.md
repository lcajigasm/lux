# Receipt: Undo/redo transaccional (operaciones invertibles)

- Phase: 01
- Crate(s): lux-core
- Goal: Transacciones y undo/redo basados en operaciones invertibles (no en
  snapshots del documento completo), con agrupación estructural e
  inteligente de ediciones consecutivas, y undo que restaura también las
  posiciones de los cursores.
- Verify: `cargo test -p lux-core --test undo_proptest -- --test-threads=1`
- Iterations: 3 / 10
- Result: passed
- Opened: 2026-07-27
- Closed: 2026-07-27
- Notes:

  **Sustrato mínimo necesario.** Este entregable no puede probarse sin que
  `Buffer` tenga *alguna* forma de mutarse — hasta ahora era solo carga y
  guardado. Se añadieron exactamente dos primitivas a `buffer/mod.rs`:
  `insert(char_idx, text)` y `delete(char_range) -> String`, más
  `text_range`/`len_chars` como lectura auxiliar. Deliberadamente **no** se
  construyeron las operaciones de edición de alto nivel del entregable 5
  (duplicar línea, comentar, unir líneas, etc.) — esas compondrán estas dos
  primitivas más adelante; añadirlas aquí habría sido salirse del alcance.

  **Diseño, tres piezas nuevas:**

  - `edit.rs` — `Edit::{Insert, Delete}`, cada uno capaz de invertirse a sí
    mismo (`invert()`) sin volver a tocar el buffer, porque un `Delete`
    guarda el texto que borró en el momento de construirse
    (`Edit::delete(buffer, char_idx, len)` lee el rango antes de aplicarlo).
  - `history.rs` — `Transaction` (una lista de `Edit` + selecciones antes y
    después, para que deshacer también restaure cursores) e `History`
    (pilas de undo/redo). `Transaction::apply` aplica en orden;
    `Transaction::undo` invierte y aplica en orden **inverso** — el
    invariante estándar de "deshacer es reproducir los inversos de atrás
    hacia delante", no un truco nuevo.
  - `tests/undo_proptest.rs` — el property test que exige el gate: genera un
    texto inicial y una secuencia de hasta 30 operaciones Insert/Delete con
    offsets crudos (`usize` sin acotar) que se recortan (`% len_chars`) al
    rango válido del buffer en el momento de aplicarse, para que cada caso
    generado sea una edición legal. Tras aplicarlas, deshace todo y verifica
    texto y cursor exactos al estado original; rehace todo y verifica texto y
    cursor exactos al estado justo antes de deshacer. También afirma en cada
    paso que el cursor nunca cae fuera de `[0, len_chars]`. 2000 casos
    aleatorios (`PROPTEST_CASES=2000`) pasan limpio, además del caso por
    defecto (256).

  **Agrupación "inteligente" sin reloj.** El prompt maestro pide
  "agrupación temporal e inteligente", pero agrupar por tiempo transcurrido
  real (`if now - last > umbral`) es exactamente el antipatrón de polling
  que la regla 5 de `CLAUDE.md` prohíbe, y además introduce no-determinismo
  en algo que debe ser probable con property tests. Se implementó en su
  lugar una heurística puramente estructural (`should_coalesce` en
  `history.rs`): dos transacciones de un solo cursor y una sola edición se
  fusionan si la segunda continúa exactamente donde terminó la primera —
  escritura consecutiva hacia delante, o borrado consecutivo en la misma
  posición (tecla Delete sostenida) o justo antes (tecla Backspace
  sostenida). Cualquier otra combinación (transacciones multicursor, saltos
  no adyacentes, mezclas de insertar/borrar) nunca se fusiona. Documentado
  en el docstring del módulo como decisión deliberada, no como limitación.

  **Fallo real durante el loop (iteración 2).** El primer diseño de
  `should_coalesce` exigía que la transacción previa tuviera exactamente
  una edición (`prev.edits.len() == 1`) para poder fusionarse. Esto rompía
  cadenas de más de dos pasos: al fusionar la primera vez, `prev.edits`
  pasaba a tener 2 elementos, y la siguiente fusión ya no calificaba —
  escribir "hi!" solo agrupaba "h"+"i" y dejaba "!" como paso de undo
  aparte. Además, con esa comprobación por longitud no había forma de
  distinguir "una cadena ya fusionada" de "una transacción multicursor
  genuina de 2 ediciones" — ambas tienen `edits.len() == 2`. Se corrigió
  añadiendo un campo explícito `chainable: bool` a `Transaction` (`true`
  solo para las construidas con el nuevo constructor `Transaction::single`,
  que representa una edición de un solo cursor; `false` para
  `Transaction::new`, la vía general/multicursor), y comparando siempre
  contra `prev.edits.last()` en vez de exigir longitud 1 — así una cadena
  puede crecer indefinidamente y una transacción multicursor nunca se
  fusiona con nada, sin importar si sus offsets coinciden por casualidad con
  los de una edición posterior.

  **Segundo fallo real (iteración 3, el que encontró el proptest).** La
  primera versión de `tests/undo_proptest.rs` no usaba el cursor que iba
  arrastrando de una operación a la siguiente: para cada `Delete`,
  reconstruía `selections_before` a partir de la propia geometría del
  borrado (`char_idx + len`) en vez de usar el cursor real heredado de la
  operación anterior. proptest encontró el caso mínimo `initial = " "`,
  `ops = [Delete(0, 0)]` en el primer intento y lo redujo automáticamente a
  ese caso de un solo carácter. La causa raíz no era `History` ni `Edit`
  sino el arnés de test mismo, inventando un `before` que no correspondía al
  cursor real. Se corrigió pasando literalmente el parámetro `cursor`
  (arrastrado desde la operación anterior, o la posición inicial en la
  primera) como `selections_before` de cada transacción — que es además la
  advertencia de "variable no usada" que el compilador ya señalaba antes de
  ejecutar el test, y que debí atender antes de correr el gate. El caso
  fallido queda persistido en
  `crates/lux-core/tests/undo_proptest.proptest-regressions` (práctica
  recomendada por `proptest`, se deja versionado).

  **Para promover a `CLAUDE.md` o memoria**: la advertencia del compilador
  sobre una variable no usada en un test resultó ser la primera señal del
  bug real (el parámetro `cursor` ignorado) — vale la pena tratar los
  warnings de test harnesses con la misma seriedad que los del código de
  producción antes de fiarse de un resultado en verde. No se considera lo
  bastante genérico como para justificar una entrada de memoria nueva por sí
  solo; queda documentado aquí por si se repite en un futuro entregable.

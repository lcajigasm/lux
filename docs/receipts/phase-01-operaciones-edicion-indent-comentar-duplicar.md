# Receipt: Operaciones de edición (indent, comentar, duplicar, ordenar...)

- Phase: 01
- Crate(s): lux-core
- Goal: Todas las operaciones de edición del entregable 5 del prompt maestro
  — insertar, borrar, borrar palabra/línea, duplicar línea, mover línea
  arriba/abajo, indentar/desindentar respetando la configuración,
  comentar/descomentar línea y bloque, unir líneas, transponer, ordenar
  líneas, cambio de caja — implementadas sobre `Buffer`, `Edit`, `Position`
  y `Selection` ya existentes, sin tocar nada fuera de este entregable.
- Verify: `cargo test -p lux-core edit_ops::`
- Iterations: 6 / 10
- Result: passed
- Opened: 2026-07-27
- Closed: 2026-07-27
- Notes:

  **Diseño.** Directorio `edit_ops/` (como `buffer/`), seis ficheros:
  `mod.rs` (helpers compartidos + `insert_text`/`delete_selection` +
  `apply_to_each`), `word.rs` (borrar palabra), `lines.rs` (borrar/duplicar/
  mover/unir/ordenar línea), `indent.rs` (indentar/desindentar),
  `comment.rs` (comentar línea y bloque), `transform.rs` (transponer, cambio
  de caja). Cada operación tiene la misma forma:
  `fn(&mut Buffer, Selection, ...) -> (Vec<Edit>, Selection)` — muta el
  buffer directamente y devuelve las ediciones hechas más el cursor
  resultante; ninguna toca `History` (eso es responsabilidad de quien
  llame). `apply_to_each` aplica una operación a los N cursores de un
  `MultiCursor` en una sola `Transaction`, atando el cabo suelto que los
  entregables de multicursor y undo/redo habían dejado explícitamente para
  "cuando existan operaciones de edición reales" — ahora existen.

  Para "borrar palabra" se implementó un clasificador de tres categorías
  (palabra/espacio/otro) mínimo, sin las reglas de subpalabra que
  corresponden al entregable de movimiento (fuera de alcance aquí).
  "Comentar/descomentar" recibe el token de comentario como parámetro
  (`"//"`, `"#"`, `"/*"`/`"*/"`) porque `lux-core` no tiene aún awareness de
  lenguaje/sintaxis (llega con tree-sitter en una fase posterior).

  **Iteraciones y fallos reales — no fue trivial, seis rondas de fallos
  distintos:**

  1. **43 errores de borrow checker.** El patrón
     `func(&mut buffer, helper(&buffer, ..))` no compila en Rust: los
     borrows de dos fases no cubren un `&mut` explícito pasado como
     argumento normal (solo aplican a auto-ref de llamadas a método). Se
     corrigió con un script que separa cada llamada en
     `let sel = helper(&buffer, ..); func(&mut buffer, sel);` en los ~30
     sitios afectados de los tests.
  2. **`ropey::Chars` no implementa `DoubleEndedIterator`.** Es un cursor
     por trozos, no un iterador de slice; `.rev()` no compila. `word.rs` se
     reescribió usando el método propio `.prev()` de `Chars` (vía
     `Rope::chars_at`), que sí existe para recorrer hacia atrás.
  3. **Aritmética de offsets equivocada en `toggle_block_comment`** (rama
     de envolver con `/* */`): usé `end.char + start_token.len() +
     end_token.len()` para el nuevo final de la selección; el token de
     cierre se inserta en el propio `end.char`, así que no desplaza esa
     misma posición — el desplazamiento correcto es solo
     `+ start_token.len()`. Encontrado y corregido a mano antes de compilar
     por primera vez, verificando la aritmética dígito a dígito.
  4. **Diez fallos reales tras la primera compilación limpia**, todos con
     causa raíz en una misma trampa recurrente:
     - `apply_to_each` calculaba la posición final de cada cursor justo
       después de su propia edición, sin contar que ediciones posteriores
       (más a la izquierda, procesadas después al ir de atrás hacia
       delante) vuelven a desplazar esa posición. Se corrigió difiriendo el
       cálculo final: cada iteración guarda el offset crudo y cuánto se
       había desplazado hasta ese momento, y solo al terminar el bucle se
       resuelve la posición definitiva con el desplazamiento total.
     - `duplicate_line`, `move_line_up`/`move_line_down` en la última línea
       de un documento sin salto de línea final: la primera versión pegaba
       el terminador en el sitio equivocado (al final de la copia en vez de
       entre el original y la copia), y el "shift" devuelto por el swap
       usaba la longitud del texto equivocado (el del bloque en vez del de
       la línea vecina, o viceversa según la dirección). Se corrigió
       devolviendo explícitamente dónde queda el contenido de *cada* mitad
       tras el intercambio, en vez de intentar derivar un único número de
       desplazamiento genérico.
     - `sort_lines` reordenaba los pares (contenido, terminador) juntos, así
       que una línea sin terminador (por ser la última del documento) se
       llevaba esa propiedad consigo a su nueva posición en vez de que la
       última línea *tras ordenar* fuera la que se quedara sin terminador.
       Se separó el contenido de la posición del terminador: todas las
       líneas salvo la nueva última llevan el EOL del buffer; la nueva
       última lleva terminador solo si el bloque original lo tenía.
     - `join_lines` calculaba cuántas uniones eran posibles a partir de
       `len_lines()` sin tener en cuenta que `ropey` cuenta una línea
       fantasma vacía cuando el texto termina en salto de línea — así que
       "unir" la última línea real intentaba unirla con esa línea vacía
       inexistente. Se corrigió contando líneas realmente unibles con un
       sondeo explícito (`¿el final de esta línea es ya el final real del
       documento?`) en vez de fiarse de `len_lines()` a secas. Mismo defecto
       de fondo (línea fantasma) afectaba a `indent`, `dedent`,
       `delete_line`, `duplicate_line`, `move_line_up`/`down`, `sort_lines`
       y `toggle_line_comment` — todas usaban `start.line..=end.line` de la
       selección directamente. Se centralizó en un solo helper,
       `touched_lines`, que excluye esa línea fantasma cuando el final de
       una selección multilínea no contiene ningún carácter real de ella, y
       se sustituyó en las siete funciones a la vez en vez de parchear cada
       una por separado.
     - El test de round-trip de `toggle_block_comment` fallaba porque la
       propia función, al envolver, devuelve una selección que cubre solo
       el contenido (sin los tokens insertados) — diseño deliberado para
       que la selección del usuario no incluya de repente sintaxis de
       comentario — así que reutilizar esa selección tal cual en una
       segunda llamada nunca detecta que ya está comentado. No era un bug
       de la función sino una asunción incorrecta del test; se corrigió el
       test para expandir la selección sobre los tokens antes de la segunda
       llamada, documentando el contrato en el propio test.

  **Para promover a memoria/CLAUDE.md**: el patrón `f(&mut x, g(&x, ..))` no
  compila en Rust y no es evidente por qué a quien no conoce los borrows de
  dos fases a fondo — vale la pena recordarlo la próxima vez que se escriban
  muchos tests de este estilo, para no repetir las 43 correcciones. La
  "línea fantasma" de `ropey` al final de un documento terminado en salto de
  línea es otra trampa con la que probablemente se vuelva a tropezar en
  fases futuras (movimiento de cursor, renderizado) si no se centraliza el
  criterio.

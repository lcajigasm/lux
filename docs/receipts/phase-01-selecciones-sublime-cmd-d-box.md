# Receipt: Selecciones a la Sublime (Cmd+D, box selection...)

- Phase: 01
- Crate(s): lux-core
- Goal: Las seis operaciones del entregable 7 del prompt maestro: expandir
  selección a palabra, seleccionar siguiente ocurrencia (Cmd+D), seleccionar
  todas las ocurrencias, saltar la ocurrencia actual, selección por
  columnas/box, y dividir selección en líneas — sobre `Buffer`, `Position`,
  `Selection` y `MultiCursor` ya existentes, sin mutar el buffer.
- Verify: `cargo test -p lux-core selection_ops::`
- Iterations: 2 / 10
- Result: passed
- Opened: 2026-07-27
- Closed: 2026-07-27
- Notes:

  **Diseño.** Directorio `selection_ops/`, cuatro ficheros: `word.rs`
  (expandir a palabra), `occurrence.rs` (Cmd+D / todas las ocurrencias /
  saltar actual), `box_select.rs` (selección por columnas), `split.rs`
  (dividir selección en líneas). Ninguna de estas funciones muta el buffer
  — todas leen `&Buffer` y devuelven un `Selection` o `MultiCursor` nuevo;
  la distinción con `edit_ops` (que sí muta) queda documentada en el
  docstring del módulo.

  `expand_to_word` es deliberadamente una implementación nueva, no una
  reutilización del escáner de límites de palabra de `edit_ops::word`: ese
  otro salta espacio en blanco antes de buscar el límite (correcto para
  "borrar la palabra anterior"), mientras que "expandir a palabra" necesita
  decidir primero a qué *clase* de tramo pertenece el cursor (para que
  hacer doble clic sobre espacios en blanco seleccione ese tramo de
  espacios, no una palabra vecina) — un algoritmo genuinamente distinto, no
  una duplicación gratuita.

  `select_next_occurrence`/`select_all_occurrences`/
  `skip_current_occurrence` usan búsqueda literal de subcadena
  (`str::match_indices`) sobre `buffer.text()`; nada de regex ni
  insensibilidad a mayúsculas — eso pertenece al entregable de buscar y
  reemplazar (fuera de alcance). Si el cursor primario está colapsado, se
  expande primero a la palabra que toca (vía `expand_to_word`), replicando
  el `Cmd+D` real de Sublime/VSCode.

  `box_select` reutiliza `Position::next_grapheme` (del entregable de
  posiciones) para caminar columna a columna respetando clusters de
  grafemas en vez de contar bytes o chars, y se detiene sin cruzar a la
  siguiente línea cuando una línea es más corta que la columna pedida — así
  que nunca hace falta reinventar el recorrido por clusters.

  `split_into_lines` reutiliza directamente `touched_lines`, `line_range` y
  `split_terminator` de `edit_ops` (son `pub(crate)`, visibles en todo el
  crate) en vez de rederivar la lógica de la "línea fantasma" de `ropey` que
  costó varias iteraciones depurar en el entregable anterior.

  **Único fallo real**: un error mío de aritmética en un test, no en el
  código — calculé a mano el rango de la tercera ocurrencia de `"foo"` en
  `"foo bar foo baz foo"` como `17..20` cuando en realidad es `16..19`
  (conté un carácter de más). Confirmado con un script Python de una línea
  y corregido en el test; la función bajo prueba (`select_all_occurrences`)
  era correcta desde el principio.

  Nada más que amerite promoción a `CLAUDE.md`.

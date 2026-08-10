# Receipt: Auto-indent por lenguaje (entregable 5)

- Phase: 03
- Crate(s): lux-syntax
- Goal: El entregable "Auto-indent por lenguaje" de la Fase 3 ("Sintaxis,
  navegación y paleta") de `LUX_PROMPT_MAESTRO.md` está completo y su gate
  pasa sin ningún fallo ni test ignorado.
- Verify: `cargo test -p lux-syntax indent_rules::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-08-08
- Closed: 2026-08-08
- Notes:

  **Diseño.** `crates/lux-syntax/src/indent_rules.rs`, nuevo módulo,
  siguiendo el mismo patrón que `brackets.rs`/`folding.rs`: opera
  directamente sobre `tree.root_node()`, sin dependencia de `lux-core` ni
  de ninguna UI. `indent_level(&Document, source, row) -> usize` devuelve
  el nivel de indentación (no espacios/tabs — esa conversión sigue siendo
  responsabilidad de `lux-core::buffer::Indent`, que ya existe y no se
  tocó) contando cuántos de los "nodos de cuerpo" del lenguaje (un
  `block`, un `compound_statement`, un `object`, ...) encierran la fila
  objetivo.

  **Por qué no es "contar todos los ancestros".** La primera idea —contar
  filas de inicio distintas entre los ancestros del punto— funciona en
  Rust/C (el header y su `{` comparten fila, así que se deduplican solos)
  pero da resultados incorrectos en Python (`if_statement` empieza en la
  fila del `if`, su `block` hijo empieza una fila después: son el mismo
  nivel de indentación pero cuentan como dos) y en Swift (`function_body`
  y su `statements` interno tampoco coinciden de fila). La solución que sí
  generaliza: cada gramática tiene un conjunto fijo y pequeño de *kinds*
  que representan "un cuerpo indentado" — establecido inspeccionando el
  árbol real de cada gramática vendorizada (igual que
  `queries/<lang>/highlights.scm` establece los nombres de captura de cada
  una), no adivinado. Los nodos contenedores/cabecera (`if_expression`,
  `else_clause`, `elif_clause`...) nunca están en la lista, así que nunca
  suman nivel por sí solos — lo cual además hace que un `else`/`elif` en
  Python dedente correctamente sin ningún caso especial por palabra clave.

  **Cierre de cuerpo (`}`, `fi`, `done`).** Una línea cuyo primer token no
  en blanco es literalmente el último token del cuerpo que la encierra
  dedenta al nivel de la línea que abrió ese cuerpo, no al nivel de
  dentro. Detectado de forma genérica (sin lista de caracteres de cierre
  hardcodeada): el nodo hoja encontrado es hijo de un nodo cuyo *kind*
  está en la lista de "cuerpo" del lenguaje y termina exactamente donde
  termina ese padre (`parent.end_byte() == leaf.end_byte()`). Esto cubre
  tanto llaves (`}`/`)`/`]`) como palabras clave de cierre (`fi` en Bash,
  `done` en Bash) con la misma regla.

  **Corrección YAML/Markdown.** Ambas gramáticas envuelven el contenido de
  nivel superior del fichero en el mismo *kind* que usan para anidamiento
  real (`block_mapping`/`block_sequence` en YAML, `list` en Markdown) —
  aunque ese nivel superior no tiene delimitador de apertura y sus hijos
  directos están en columna 0, igual que el propio fichero. Se descubrió
  al escribir el test de YAML: una clave de primer nivel salía con nivel 1
  en vez de 0. JSON/HTML no tienen este problema (su nodo más externo
  tiene un `{`/`<div>` real y su contenido sí está indentado un nivel).
  Corregido restando 1 solo para esos dos lenguajes (`root_offset`),
  documentado en el propio código con la razón, no como un `#[allow]`
  silencioso.

  **Verificación empírica antes de codificar.** En vez de adivinar nombres
  de *kind* por lenguaje, se añadió temporalmente un test de sondeo
  (`probe_tmp`, eliminado antes de terminar) que volcaba el árbol real de
  cada gramática vendorizada para muestras representativas (Rust, Python,
  C/C++, Java, JS/TS, Go, Kotlin, Swift, HTML, CSS, JSON, YAML, Markdown,
  Bash, TOML), incluyendo casos de clase+método y bash con
  if/elif/else/fi + for/do/done. Los 18 conjuntos de `block_kinds` en el
  código final vienen de esa inspección real, no de la documentación de
  cada gramática.

  **Límite conocido y documentado (no oculto).** El gramático de
  tree-sitter-bash no envuelve el cuerpo principal de un `if`/`then` en un
  nodo propio — `if_statement` hace doble papel de cabecera y cuerpo, y
  sus cláusulas `elif`/`else` son hermanas suyas, no anidadas dentro de
  otro *kind* contable. Esto significa que el cuerpo de `if`/`for` y el
  cierre `fi`/`done` calculan bien (verificado por tests), pero
  redentar la propia línea `elif`/`else` al nivel del `if` no está
  derivado especialmente — se deja documentado con un comentario
  `ponytail:` en el código en vez de construir un mecanismo de
  clave-de-rama solo para esta gramática. Ningún otro de los 18 lenguajes
  tiene esta limitación.

  **Verificación real.** `cargo test -p lux-syntax indent_rules::`: 18/18
  passed, 0 ignorados. Los tests no son triviales: cubren anidamiento
  real de profundidad en Rust/C con cabecera y llave en la misma fila,
  bloque Python sin ninguna llave, `else` de Python dedentando sin caso
  especial, un `{` dentro de un string literal (nunca contado como
  bracket real), líneas en blanco dentro de un cuerpo, cierre de llave
  dedentando a la línea que abrió, Bash `if/then/fi` y `for/do/done`
  completos, JSON/YAML anidados, y la corrección de YAML/Markdown descrita
  arriba. Gate completo del repo en verde: `cargo run -p xtask` (fmt,
  clippy `-D warnings` en el workspace entero, `cargo test --workspace`:
  235 tests en `lux-core` + el resto del workspace sin regresiones,
  `cargo deny check`, límites de fichero, aislamiento de `lux-core`). Los
  cuatro entregables anteriores de esta fase (`highlight::`, `theme::`,
  `folding::`, `brackets::`) se re-verificaron intactos en la misma
  pasada. `crates/lux-syntax/src/indent_rules.rs` tiene 276 líneas, bien
  lejos del límite de 600; no se añadió ningún campo a ningún struct ni
  ninguna dependencia nueva — solo un método `pub(crate) fn language()`
  en `Document`, análogo al `tree()` que ya existía para `brackets`/
  `folding`.

  **Qué queda fuera de este entregable** (resto de la Fase 3, sin tocar):
  Goto Anything, paleta de comandos, barra de búsqueda/reemplazo,
  pestañas/barra de estado, minimapa — y, dentro del propio alcance de
  auto-indent, el wiring hacia un comando real de editor (tecla Enter,
  reindentar selección) en `lux-ui`, que no existe todavía en esta fase
  (no hay UI de edición que consuma esto aún) y por tanto es trabajo
  posterior, no de esta librería. Verificado solo en macOS, misma
  limitación documentada en el resto de `docs/STATUS.md`.

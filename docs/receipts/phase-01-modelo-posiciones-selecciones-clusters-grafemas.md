# Receipt: Modelo de posiciones/selecciones con clusters de grafemas

- Phase: 01
- Crate(s): lux-core
- Goal: Modelo de posiciones y selecciones robusto: distinción explícita
  entre índice de byte, índice de carácter y columna visual; manejo correcto
  de clusters de grafemas (no rompe emojis ni acentos combinantes).
- Verify: `cargo test -p lux-core -- position:: selection::`
- Iterations: 2 / 8
- Result: passed
- Opened: 2026-07-27
- Closed: 2026-07-27
- Notes:

  **Nota sobre el comando de verificación literal.** El comando indicado en
  el objetivo, `cargo test -p lux-core position:: selection::`, no es
  ejecutable tal cual: `cargo test` solo acepta un único argumento posicional
  `TESTNAME` sin `--`; pasarle un segundo (`selection::`) es un error de
  parseo de argumentos de `cargo` (`unexpected argument 'selection::'
  found`), no un fallo de test. Para pasar dos filtros de nombre al binario
  de tests hace falta el separador `--`:
  `cargo test -p lux-core -- position:: selection::`. Esa es la forma que se
  usó para verificar (y la que se deja documentada arriba como `Verify`).
  También se comprobó cada filtro por separado
  (`cargo test -p lux-core -- position::` y
  `cargo test -p lux-core -- selection::`) para descartar que el filtro
  combinado ocultara algo.

  **Diseño.** Dos módulos nuevos en `lux-core`, hermanos de `buffer`:

  - `position.rs` — `Position { byte, char, line, column }`. `byte`/`char`
    son índices absolutos en el documento (offset de byte UTF-8 vs. offset
    de char / valor escalar Unicode — deliberadamente separados para no
    repetir el bug clásico de mover "un carácter" y en realidad partir un
    carácter multibyte). `column` es el número de *clusters de grafemas*
    desde el inicio de línea, no de bytes ni de chars. `next_grapheme` /
    `prev_grapheme` mueven un cluster completo usando
    `unicode_segmentation::GraphemeCursor` alimentado trozo a trozo desde
    `Rope::chunk_at_byte` (el patrón documentado de `ropey` para esto), en
    vez de materializar el documento entero en cada paso — importante para
    no introducir un `O(n)` por pulsación de tecla en archivos grandes.
  - `selection.rs` — `Selection { anchor, head }` de una sola selección
    (anchor/head), no multicursor (eso es el entregable 3, fuera de
    alcance). Incluye `is_reversed`, `char_range`/`byte_range` ordenados
    independientemente de la dirección, `collapse_to_head`, y `overlaps`
    para detectar solapamiento de rango (necesario más adelante para fusionar
    selecciones al implementar multicursor).

  **Dependencia añadida.** `unicode-segmentation` 1.13.3 — la stdlib de Rust
  no ofrece segmentación en clusters de grafemas (solo `chars()`, que itera
  valores escalares Unicode), así que hace falta una crate para esto. No
  estaba en la tabla de `docs/DEPENDENCIES.md` porque el prompt maestro no la
  menciona explícitamente; no se tocó ese fichero por estar fuera del alcance
  de este entregable (documentar la nueva dependencia queda anotado aquí).

  **Para mantener `ropey` fuera de la API pública** (ADR 0007), se añadió
  `pub(crate) fn rope(&self) -> &Rope` a `Buffer` — `position.rs` y
  `selection.rs` lo usan internamente, pero ninguna firma pública de
  `lux-core` expone tipos de `ropey`.

  Sin bloqueos ni fallos repetidos más allá de la discrepancia del comando
  documentada arriba. No hay nada aquí que amerite promoción a `CLAUDE.md`.

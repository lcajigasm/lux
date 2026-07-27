# Receipt: lux-core-cli (arnés de pruebas headless)

- Phase: 01
- Crate(s): lux-core-cli (nuevo)
- Goal: Un binario `lux-core-cli` que abre un fichero, aplica un guion de
  operaciones y vuelca el resultado — prueba end-to-end del núcleo sin UI,
  y prueba viva de que `lux-core` no depende de ninguna.
- Verify: `cargo run -p lux-core-cli -- --script tests/fixtures/basic.script`
- Iterations: 2 / 5
- Result: passed
- Opened: 2026-07-27
- Closed: 2026-07-27
- Notes:

  **El crate no existía.** A diferencia de otros entregables de esta fase,
  `lux-core-cli` no tenía stub de la Fase 0 (esa fase creó los crates de la
  tabla de dependencias de 0.6, y `lux-core-cli` es un entregable propio de
  la Fase 1, listado ahí por primera vez). Se creó
  `crates/lux-core-cli/` completo: `Cargo.toml` (un solo binario, una sola
  dependencia — `lux-core` por path) y `src/main.rs`.

  **Diseño del guion.** Un lenguaje de comandos línea por línea, deliberada
  y mínimamente potente — sin parser combinator ni gramática formal, solo
  `split_once` sobre espacios — porque su único trabajo es ejercitar
  `lux-core` desde fuera, no ser un lenguaje de scripting real:

  - `open <path>` — carga un fichero (ruta relativa al directorio del
    propio guion, no al cwd, para que los guiones sean reubicables) como
    buffer activo.
  - `insert <char_idx> <texto>` / `delete <char_idx> <len>` — ediciones
    primitivas, cada una registrada en un `History` real vía
    `Transaction::single`.
  - `undo` / `redo` — ejercitan `History` de verdad, no solo `Buffer`.
  - `dump` — imprime el texto del buffer a stdout.
  - `assert_eq <texto>` — compara el buffer contra el resto de la línea y
    sale con código de error y un mensaje claro si no coincide. Sin esto,
    el arnés solo "volcaría" texto sin poder fallar nunca — con esto, el
    propio guion es una prueba automatizada de verdad, no solo un volcador.

  `tests/fixtures/basic.script` abre `basic.txt` ("hello world", 11 bytes,
  sin salto de línea final), inserta una coma, la borra, deshace, rehace, y
  hace `assert_eq` después de cada paso — ejercitando `Buffer`, `Edit`,
  `Transaction` e `History` juntos en una sola pasada. Verifiqué también,
  con un guion desechable fuera del repositorio, que un `assert_eq` que no
  debe cumplirse sí produce código de salida distinto de cero y un mensaje
  con el texto esperado y el real — para no descubrir después de cerrar
  este entregable que el arnés nunca podía fallar.

  **Único fallo real**: dos usos de `session.history.undo(buffer)` /
  `.redo(buffer)` donde `buffer` venía de una función auxiliar
  `current_buffer(session)` que devuelve `&mut Buffer` atado al préstamo
  de todo `&mut Session` — el borrow checker no puede ver, a través de una
  llamada a función, que `buffer` y `history` son campos disjuntos, así
  que pedir `&mut session.history` en la misma expresión mientras `buffer`
  seguía vivo era un préstamo mutable duplicado. Se corrigió
  desestructurando `Session { buffer, history } = session` directamente en
  esos dos casos (no en `insert`/`delete`, donde no hacía falta: ahí el
  préstamo de `buffer` ya había terminado, por último uso, antes de la
  llamada a `history.record`). El mismo patrón de "función auxiliar rompe
  el análisis de préstamos disjuntos por campo" ya apareció de forma
  distinta en entregables anteriores; aquí se reconoció y resolvió al
  primer intento de compilación.

  Nada que amerite promoción a `CLAUDE.md`.

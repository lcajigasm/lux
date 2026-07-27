# Receipt: Movimiento (entregable 6)

- Phase: 01
- Crate(s): lux-core
- Goal: Movimiento de cursor — carácter, palabra (con reglas de
  subpalabra), línea, párrafo, página, inicio/fin de línea inteligente,
  inicio/fin de documento, con y sin selección; emparejamiento de brackets
  y salto entre pares. El único de los 10 entregables de la Fase 1 que
  había quedado sin abordar (detectado durante `/close-phase 1`).
- Verify: `cargo test -p lux-core movement::`
- Iterations: 2 / —
- Result: passed
- Opened: 2026-07-27
- Closed: 2026-07-27
- Notes:

  **Diseño.** Directorio `movement/`: `word.rs` (subpalabra), `line.rs`
  (línea + inicio/fin inteligente), `paragraph.rs`, `page.rs`, `bracket.rs`,
  y `mod.rs` (carácter, inicio/fin de documento, y `extend_or_move`). Cada
  función de movimiento tiene la forma `fn(&Buffer, Position, ...) ->
  Position` — no toca `Selection` ni `MultiCursor` directamente.
  `extend_or_move(current, to, extend)` es el único sitio que decide si un
  destino colapsa el cursor o extiende la selección actual (`Shift+tecla`
  vs. tecla sola), así que ningún movimiento individual repite esa lógica —
  es el "con y sin selección" del entregable resuelto una sola vez, no N
  veces.

  - **Carácter**: envoltorio trivial sobre `Position::next_grapheme`/
    `prev_grapheme` (ya construidos en el entregable de posiciones) — un
    carácter de movimiento es un clúster de grafemas, no un `char` de Rust.
  - **Palabra con reglas de subpalabra**: clasificador nuevo y distinto del
    de `edit_ops::word` (que salta espacio en blanco primero, correcto para
    "borrar palabra anterior" pero no para "moverse"). Detecta transiciones
    `camelCase`/`PascalCase` (incluyendo el caso `HTTPRequest` → `HTTP` |
    `Request`, mirando un carácter por delante) y trata `_`/`-` como
    separadores propios, no como parte de la palabra.
  - **Línea**: sube/baja preservando una "columna objetivo" que el
    *llamador* mantiene entre pulsaciones consecutivas (no hay estado
    oculto en `lux-core` para esto) — así que atravesar una línea corta y
    volver a una larga no pierde la posición horizontal original.
  - **Página**: `lux-core` no tiene ninguna noción de viewport (tamaño de
    fuente, alto de ventana, ajuste de línea — todo eso es UI, regla 2 de
    `CLAUDE.md`), así que `lines_per_page` es un parámetro que pone quien
    llama, no algo que el núcleo intente adivinar.
  - **Inicio de línea inteligente**: alterna entre la primera columna no
    en blanco y la columna 0, como Sublime/VS Code.
  - **Párrafo**: salta al límite de línea en blanco más cercano (arriba o
    abajo), la misma noción que `vim` `}`/`{`.
  - **Emparejamiento de brackets**: conteo de profundidad de anidamiento
    sobre `()`/`[]`/`{}`, sin conciencia de sintaxis — `lux-core` no tiene
    tokenizador todavía (llega con tree-sitter en una fase posterior), así
    que un bracket dentro de una cadena de texto se cuenta igual que uno
    real. Documentado explícitamente como limitación honesta, no oculta.

  **Fallo real**: `move_paragraph_prev` tenía la condición de la segunda
  fase del escaneo invertida (`!is_blank_line(buffer, line - 1)` en vez de
  `!is_blank_line(buffer, line)`), lo que hacía que saltara de más — en
  vez de aterrizar en la línea en blanco justo encima del párrafo actual,
  seguía hasta el inicio del párrafo anterior a esa línea en blanco. El
  test `prev_paragraph_stops_at_the_blank_line_above` lo detectó al primer
  intento de compilación; se corrigió reescribiendo la condición para que
  fuera simétrica con `move_paragraph_next` (que sí aterriza correctamente
  en la línea en blanco, verificado por sus propios tests).

  40/40 tests nuevos en verde; el crate completo pasa de 195 a 235 tests
  (sumando también los dos de regresión del fix de `apply_to_each` de este
  mismo turno). `fmt`, `clippy -D warnings` y `check-limits.sh` limpios.

  Con esto quedan cubiertos los 10 entregables de la Fase 1. Sigue
  pendiente el resto del cierre de fase: benchmarks de rendimiento,
  compilación cruzada a iOS/Android, y la corrida formal de
  `scripts/check-core-isolation.sh` — ninguno de los tres se abordó en este
  goal.

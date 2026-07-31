# Receipt: `lux-syntax` — tree-sitter incremental, gramáticas fijas (entregable 1)

- Phase: 03
- Crate(s): lux-syntax
- Goal: El entregable "lux-syntax: tree-sitter incremental, 15 lenguajes" de
  la Fase 3 ("Sintaxis, navegación y paleta") está completo y su gate pasa
  sin ningún fallo ni test ignorado.
- Verify: `cargo test -p lux-syntax highlight:: --test golden_tokens`
- Iterations: ~10 / 12
- Result: passed
- Opened: 2026-07-31
- Closed: 2026-07-31
- Notes:

  **Sobre el número "15" en el nombre del entregable.** No coincide con
  ninguna fuente del proyecto. `LUX_PROMPT_MAESTRO.md` y
  `docs/adr/0008-tree-sitter-grammars-compiled-in.md` enumeran el mismo
  conjunto fijo en prosa como "17" (Rust, TypeScript/TSX, JavaScript, Python,
  Go, C, C++, Java, Kotlin, Swift, HTML, CSS, JSON, YAML, Markdown, shell y
  TOML), pero TypeScript y TSX son dos gramáticas tree-sitter distintas
  (`tree_sitter_typescript` y `tree_sitter_tsx`, cada una con su propio
  `parser.c` de ~8.7 MB) — así que el conjunto real es **18**. No hay ningún
  documento del repo que diga "15". Ante esta discrepancia y la regla 7
  (alcance cerrado, no se reduce ni se amplía por iniciativa propia), implementé
  el conjunto completo y ratificado por la ADR (18 gramáticas) en vez de
  adivinar cuáles 3 recortar para llegar a 15. Documentado también en
  `docs/STATUS.md` y en la fila de `tree-sitter` de `docs/DEPENDENCIES.md`
  (que además tenía una referencia rota a "ADR 0009" — corregida a 0008 de
  paso, ya que estaba editando esa misma línea).

  **Diseño.** `Document` (parser + `Option<Tree>`) implementa el reparseo
  incremental real: `edit()` aplica un `tree_sitter::InputEdit` al árbol
  anterior y vuelve a parsear pasándole ese árbol, así que tree-sitter reutiliza
  los subárboles fuera del rango editado en vez de reparsear el fichero
  entero. `BackgroundHighlighter` mueve esto a un hilo dedicado: un canal
  `mpsc` bloqueante en ambas direcciones (nada de polling, regla 5) recibe
  trabajos (`SetText`/`Edit`) y devuelve los tokens calculados; el `Drop`
  cierra primero el `Sender` (para que el `for job in job_rx` del hilo
  termine) y solo entonces hace `join()` — sin ese orden el `join` se
  bloquearía para siempre.

  **Vendorizado de gramáticas.** Siguiendo la ADR 0008 y
  `.claude/skills/add-tree-sitter-grammar/SKILL.md`: cada gramática vive en
  `crates/lux-syntax/vendor/<lang>/src/` (parser.c + scanner.c si aplica +
  headers `tree_sitter/*.h`), compilada por `build.rs` vía el crate `cc`, sin
  depender del crate `tree-sitter-<lang>` de crates.io (evita el conflicto de
  versiones del issue #3095 que motiva la ADR). 17 de las 18 gramáticas se
  descargaron directamente del repo git de origen (GitHub raw, tag por
  tag). **Swift fue la excepción**: `alex-pinkus/tree-sitter-swift` no
  commitea su `parser.c` generado (está en `.gitignore`, ~20 MB); lo extraje
  del tarball publicado en crates.io (`tree-sitter-swift-0.7.3.crate`, que sí
  empaqueta el `parser.c` generado) — solo para copiar los ficheros C, sin
  añadir el crate como dependencia de Cargo. Total vendorizado: 113 MB.

  **Tres gramáticas necesitaron ficheros extra no obvios por el listado
  superficial del repo:**
  - TypeScript/TSX comparten `common/scanner.h` en la raíz del monorepo
    (`typescript/src/scanner.c` lo incluye como `../../common/scanner.h`).
  - HTML's `scanner.c` incluye `tag.h`, un fichero hermano no listado junto a
    parser.c/scanner.c en el primer vistazo.
  - YAML's `scanner.c` incluye `schema.core.c`/`schema.json.c`/
    `schema.legacy.c` directamente (no como headers, como fuente C inline).

  Los tres se detectaron por error de compilación real (`fatal error: ...
  file not found`), no por inspección previa — cada uno costó una iteración
  de fetch-and-retry.

  **Hallazgo real sobre las queries de highlights.** Las `highlights.scm`
  que trae el propio repo de TypeScript, TSX y C++ son deliberadamente
  parciales: cubren solo las adiciones específicas de ese lenguaje sobre una
  gramática base (JavaScript para TypeScript/TSX, C para C++), el mismo
  patrón de capas que aplican nvim-treesitter/helix en su cargador de
  queries en tiempo de ejecución. Sin esa capa, el test dorado de C++ daba
  solo 4 tokens para un fichero con clases/métodos/comentarios — una señal
  clara de gramática mal servida, no de fichero vacío. Corregido componiendo
  la query final con `concat!` en tiempo de compilación (`c` + `cpp`,
  `javascript` + `typescript`, `javascript` + `typescript` + `tsx`), ya que
  las gramáticas base y extendida comparten los mismos nombres de tipo de
  nodo. Tras el fix: C++ 4 → 88 tokens, TSX 8 → 62, TypeScript 20 → 114.
  Markdown se quedó deliberadamente en el grammar de bloque únicamente
  (`tree-sitter-markdown`, no `tree-sitter-markdown-inline`) — negrita/enlaces
  a nivel de inline no están cubiertos; es una reducción de alcance
  consciente, no un bug, documentada aquí para que no se lea como un olvido.

  **Verificación real.** `cargo test -p lux-syntax highlight:: --test
  golden_tokens`: 18/18 passed, 0 ignored. Cada test parsea un fixture real
  (no trivial: struct/impl/genéricos en Rust, JSX en TSX, decoradores en
  Python, plantillas en C++, coroutines cubiertas por la keyword list en
  C++, etc.) y compara el dump exacto de tokens contra un fichero dorado
  (`LUX_SYNTAX_BLESS=1` para regenerarlo). Además del gate exacto del
  entregable: `cargo run -p xtask` completo en verde (fmt, clippy
  `-D warnings` en el workspace entero, `cargo test --workspace`, `cargo deny
  check`, `check-limits.sh`, `check-core-isolation.sh`). `cargo deny check`
  no señaló ninguna licencia ni advisory nueva por `tree-sitter`/
  `tree-sitter-language`/`cc`. Ningún fichero de la Fase 3 supera 600 líneas
  (`lib.rs` 272, `build.rs` ~50, `golden_tokens.rs` ~200 tras `cargo fmt`).

  **Qué queda fuera de este entregable** (resto de la Fase 3, sin tocar):
  temas de tokens (colores reales por captura), colorización de pares de
  paréntesis, code folding por sintaxis, auto-indent por lenguaje, y sobre
  todo — wiring de estos tokens calculados hacia `lux-ui` para pintarlos en
  pantalla. `lux-syntax` es una librería de dominio (regla 2: el core no
  sabe nada de UI); "implementado" en `docs/STATUS.md` se refiere a esa
  librería verificada por tests dorados, no a resaltado visible en el
  editor todavía. Verificado solo en macOS — mismo límite documentado en el
  resto de este fichero (sin toolchain de Windows/Linux disponible aquí). No
  se ejecutó `cargo bench` (no hay presupuesto de rendimiento definido para
  highlighting en `docs/PERFORMANCE.md` todavía; el "< 2 ms" de reparseo tras
  pulsación que cita `LUX_PROMPT_MAESTRO.md` no tiene aún un harness Criterion
  — candidato a un entregable de rendimiento posterior de esta misma fase,
  no de este).

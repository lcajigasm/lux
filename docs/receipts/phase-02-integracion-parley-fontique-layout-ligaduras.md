# Receipt: `lux-render` — integración `parley` + `fontique` (layout, ligaduras, clusters)

- Phase: 02
- Crate(s): lux-render
- Goal: El entregable "Integración parley + fontique (layout, ligaduras,
  clusters)" de la Fase 2 está completo: `lux-render` puede construir el
  layout de una línea de texto monoespaciada con `parley` + `fontique`,
  controlar features OpenType (`calt`/`liga`, ligaduras de programación) por
  rango de bytes, y exponer la geometría de sus clusters (rango de bytes,
  posición x, avance, inicio/continuación de ligadura) para que cursor,
  selección y preedit de IME la consuman más arriba en la pila. Su gate pasa
  sin ningún fallo ni test ignorado.
- Verify: `cargo test -p lux-render text_layout::`
- Iterations: 2 / 12
- Result: passed
- Opened: 2026-07-30
- Closed: 2026-07-30
- Notes:

  **Hallazgo real antes de escribir ninguna línea de código**: añadir
  `parley`/`fontique` con sus *features* por defecto rompe la resolución de
  dependencias de todo el workspace. `fontique`'s `system` (activa por
  defecto) trae `yeslogic-fontconfig-sys` 6.x; `lux-platform` ya fija (vía
  `winit` → `wayland-csd-adwaita-crossfont` → `crossfont`)
  `yeslogic-fontconfig-sys` 5.x. Ambas versiones declaran `links =
  "fontconfig"`, y Cargo prohíbe dos versiones de un mismo `links` en un
  único grafo de dependencias — y como `lux-render` y `lux-platform` son
  miembros del mismo workspace con un solo `Cargo.lock`, esto no es una
  decisión por crate. Solución: `parley` y `fontique` con
  `default-features = false, features = ["std"]` (sin `system`), y las
  fuentes se registran directamente desde bytes
  (`Collection::register_fonts`) en vez de descubrirse del sistema. Esto
  significa que **este entregable no incluye enumeración de fuentes del
  sistema** — es una laguna real y documentada (`docs/DEPENDENCIES.md`,
  `docs/STATUS.md`), no un placeholder que finge funcionar; resolverla
  exige o bien dejar de fijar `crossfont`, o resolver el conflicto de la
  librería nativa, ninguna de las dos decidible dentro de este entregable.

  **Segundo hallazgo real, en `docs/DEPENDENCIES.md`**: la re-verificación
  del 2026-07-26 registró `harfrust` 0.12.0 como lo que trae `parley`
  transitivamente. Al resolver la dependencia de verdad para este
  entregable, `Cargo.lock` fija `harfrust` 0.10.0 — el `Cargo.toml` de
  `parley` 0.11.0 pide exactamente `"0.10.0"` (caret implícito, `>=0.10.0,
  <0.11.0`), no 0.12.0. 0.12.0 es la última versión publicada de `harfrust`
  en solitario, pero no la que `parley` 0.11.0 usa. Corregido en
  `docs/DEPENDENCIES.md` con la misma convención de "Corrección" ya usada
  ahí para `grep-regex`. También aparece un aviso de versiones duplicadas
  (`read-fonts`/`skrifa`: `swash` fija 0.41.0/0.44.0, `parley`/`fontique`
  fijan 0.40.2/0.43.2) — no bloqueante (`bans.multiple-versions = "warn"`
  en `deny.toml`), documentado igualmente.

  **Diseño.** `TextLayoutContext` envuelve `parley::FontContext` +
  `parley::LayoutContext` (con un brush `NoBrush` — el color de pintado no
  es responsabilidad de esta capa). `register_font` registra bytes de
  fuente directamente y devuelve el nombre de familia que `parley` resolvió,
  para usarlo en `LineStyle::family`. `layout_line` construye un
  `RangedBuilder`, fija familia/tamaño/ligaduras por defecto con
  `push_default`, y aplica overrides de ligaduras por rango de bytes con
  `push(..., range)` — control real "por run", no solo un interruptor de
  línea completa. Las ligaduras se controlan con las dos features OpenType
  que de verdad las producen: `calt` (alternates contextuales) y `liga`
  (ligaduras estándar). `LineLayout::clusters()` recorre `line.items()` →
  `PositionedLayoutItem::GlyphRun` → `glyph_run.run().clusters()`,
  acumulando el avance desde el offset de baseline del run — la misma
  aritmética que `Cluster::visual_offset()` hace por-cluster, hecha una sola
  vez para toda la línea. Sin salto de línea forzado
  (`break_all_lines(None)`): una línea de editor nunca se envuelve a media
  línea, eso lo decide la vista virtualizada de `lux-ui`, no `parley`.

  **Fixture reutilizado, no duplicado**: los tests usan el mismo `ahem.ttf`
  (dominio público, ver `tests/fixtures/README.md`) que ya vive en el
  crate desde el entregable del atlas de glifos — cada glifo es un cuadrado
  sólido de avance fijo igual al tamaño em, ideal para afirmar avances
  monoespaciados exactos sin depender de las métricas de una fuente real.

  **Lo que los tests prueban de verdad** (`cargo test -p lux-render
  text_layout::`, 6 tests, 0 fallos, 0 ignorados):
  - Registrar Ahem resuelve al nombre de familia real que trae su propia
    tabla `name` ("Ahem"), no un nombre inventado.
  - Una línea monoespaciada de 4 caracteres produce 4 clusters, cada uno
    con el mismo avance (± 0.01px) y colocados borde con borde en una
    rejilla real — y el ancho total de la línea coincide con
    `avance × 4`.
  - Un override de ligaduras por rango de bytes se acepta y se aplica sin
    romper el resto de la línea (Ahem no define `calt`/`liga`, así que no
    hay colapso de glifos que observar de verdad — eso queda para cuando
    el editor tenga una fuente de programación real; lo que este test sí
    prueba es que la capa de control por-run no descarta ni corrompe nada).
  - Texto vacío produce cero clusters y ancho cero.
  - Doblar el tamaño de fuente dobla el avance proporcionalmente.
  - Registrar bytes que no son una fuente devuelve `None` en vez de entrar
    en pánico.

  **Fuera de alcance, a propósito**: conectar esta capa al atlas de glifos
  o a un paso de render real (ninguna letra llega a pantalla todavía),
  enumeración de fuentes del sistema (bloqueada por el conflicto de
  `fontconfig` documentado arriba), y verificación visual de una ligadura
  real (necesita una fuente de programación de verdad). No se tocó ningún
  otro crate ni ningún archivo de `lux-render` fuera de `text_layout.rs`,
  `lib.rs` (registro del módulo) y `Cargo.toml` (dependencias nuevas).

  `cargo run -p xtask` (fmt, clippy `-D warnings`, `cargo test --workspace`
  — 235 tests de `lux-core` + 12 de `lux-platform` + 21 de `lux-render` [15
  previos + 6 nuevos], `cargo deny check` — sin licencias nuevas, solo avisos
  de versiones duplicadas no bloqueantes —, límites de 600 líneas/25 campos,
  aislamiento del núcleo) en verde de punta a punta.

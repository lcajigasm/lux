# Receipt: `lux-render` — atlas de glifos (`etagere` + `swash`, 4×1 subpíxel)

- Phase: 02
- Crate(s): lux-render
- Goal: El entregable "Atlas de glifos (etagere + swash, 4x1 subpixel)" de
  la Fase 2 está completo: un atlas propio que empaqueta con `etagere` y
  rasteriza con `swash`, con el diseño 4 variantes horizontales × 1 vertical
  y texturas separadas por tipo de glifo (monocromo/policromo/subpíxel), con
  su gate en verde. El resto del entregable 2 del prompt maestro (pipeline
  de quads/texto, subida a `wgpu::Texture`, caché con desalojo,
  `TextRenderingMode`) queda fuera, para loops posteriores.
- Verify: `cargo test -p lux-render glyph_atlas::`
- Iterations: 2 / 12
- Result: passed
- Opened: 2026-07-30
- Closed: 2026-07-30
- Notes:

  **Diseño.** `GlyphAtlas` mantiene tres `Plane` (una por `GlyphKind`:
  `Monochrome`/`Polychrome`/`Subpixel`, mapeadas 1:1 a
  `swash::scale::image::Content::{Mask,Color,SubpixelMask}`), cada una con
  su propio `etagere::BucketedAtlasAllocator` (elegido sobre
  `AtlasAllocator` porque la propia documentación de `etagere` lo recomienda
  para "a very large amount of small items" — exactamente el caso de
  glifos) y su propio buffer de píxeles en CPU (1 byte/píxel para
  monocromo, 4 para policromo/subpíxel). Un `HashMap<GlyphKey, GlyphEntry>`
  cachea por (font, glyph, tamaño, fase subpíxel) para que el mismo glifo
  nunca se rasterice ni se empaquete dos veces.

  **El "4×1 subpíxel" concretamente**: la posición horizontal fraccional del
  lápiz (`x.fract()`) se cuantiza en 4 cubos (0, 0.25, 0.5, 0.75) vía
  `quantize_phase`, y ese offset se pasa a `swash::scale::Render::offset`
  antes de rasterizar — el propio ejemplo de la documentación de `swash`
  señala literalmente "you'll likely want to quantize this in a real
  renderer", que es justo lo que se implementó. No hay cuantización
  vertical (el "×1"): el offset vertical siempre es `0.0`, los baselines
  encajan en píxel entero. `Format::subpixel_bgra()` (de `zeno`,
  re-exportado como `swash::zeno::Format`) es el formato pedido por
  defecto, tal como exige `docs/adr/0003-text-rendering-stack.md`.

  **Fixture de test real, no sintético**: se descargó `ahem.ttf` (dominio
  público, ver `crates/lux-render/tests/fixtures/README.md`) desde
  `googlefonts/fontations`. Ahem es la fuente que W3C construyó
  específicamente para pruebas tipográficas — cada glifo es un cuadrado
  sólido que llena su em-box — así que rasterizar cualquier glyph id
  garantiza tinta real, no vacía, sin depender de la calidad de hinting de
  una fuente de producción. Se usa solo en tests; nunca se distribuye con
  el editor.

  **Lo que los tests prueban de verdad** (`cargo test -p lux-render
  glyph_atlas::`, 8 tests, 0 fallos, 0 ignorados):
  - `quantize_phase` cubre los 4 cubos exactos en `[0,1)` y envuelve
    fracciones negativas/fuera de rango.
  - Rasterizar la misma `GlyphKey` dos veces da cache hit real (mismo
    resultado, sin volver a empaquetar) — verificado con `is_cached`.
  - Las 4 fases subpíxel del mismo glifo aterrizan en 4 rectángulos
    distintos del atlas (rasterizado real de `swash` contra bytes reales
    de Ahem, no un stub).
  - Un glifo rasterizado con el formato subpíxel por defecto cae en el
    plano `Subpixel`, y su rectángulo empaquetado contiene tinta real no
    nula en el centro (verificado leyendo directamente el buffer RGBA).
  - Pedir `Format::Alpha` explícitamente produce `Content::Mask` real de
    `swash` y ejercita el camino de empaquetado/blit del plano
    `Monochrome`.
  - El plano `Polychrome` se ejercita con datos RGBA sintéticos (no hay
    fuente de color COLR/CBDT como fixture — añadir una solo para esto
    habría sido sobre-alcance) para confirmar que el blit de 4
    bytes/píxel coloca exactamente los píxeles esperados y no toca nada
    fuera del rectángulo destino.
  - Un plano que se queda sin espacio devuelve `None` en vez de corromper
    el empaquetado de un glifo vecino — el tamaño del plano de prueba se
    deriva del tamaño real (`width.max(height)`) del primer glifo
    rasterizado, no de un número mágico adivinado.

  **Fuera de alcance, a propósito**: subir los planos a `wgpu::Texture`
  (queda para "pipeline de quads y de texto"), desalojo de caché cuando un
  plano se llena, y `TextRenderingMode` configurable. `GlyphAtlas` expone
  `plane_bytes`/`plane_side` precisamente para que esa subida futura no
  necesite rediseñar nada de este módulo. No se tocó ningún otro crate ni
  ningún archivo de `lux-render` fuera de `glyph_atlas.rs`, `lib.rs`
  (registro del módulo) y `Cargo.toml` (dependencias nuevas).

  **`cargo deny check`**: sin hallazgos nuevos — `swash` trae `zeno`,
  `skrifa`/`read-fonts`/`font-types` (Fontations) y `yazi`; `etagere` trae
  `euclid`; todas ya cubiertas por las licencias permisivas ya presentes en
  `deny.toml`, sin necesidad de añadir ninguna entrada. `docs/DEPENDENCIES.md`
  actualizado para reflejar que ambas dependencias pasaron de "en la tabla,
  sin usar" a realmente conectadas.

  `cargo run -p xtask` (fmt, clippy `-D warnings`, `cargo test --workspace`
  — 235 tests de `lux-core` + 12 de `lux-platform` + 15 de `lux-render` [7
  previos + 8 nuevos], `cargo deny check`, límites de 600 líneas/25 campos,
  aislamiento del núcleo) en verde de punta a punta.

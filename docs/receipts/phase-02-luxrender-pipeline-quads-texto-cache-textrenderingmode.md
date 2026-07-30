# Receipt: `lux-render` — pipeline de quads/texto, caché con desalojo, `TextRenderingMode`

- Phase: 02
- Crate(s): lux-render
- Goal: El resto del entregable 2 de la Fase 2 (`lux-render`: pipeline de
  quads y de texto, caché de glifos con desalojo, `TextRenderingMode`
  configurable) está completo, con pipelines reales de `wgpu` verificados
  contra píxeles reales, no solo compilados.
- Verify: `cargo test -p lux-render` (incluye `glyph_atlas::` y el nuevo
  `tests/golden_render.rs`) + `cargo bench -p lux-render -- --baseline
  none` re-confirmado sin regresión
- Iterations: ~6 (2 errores reales de API de `wgpu` 30 —
  `PipelineLayoutDescriptor` cambió `push_constant_ranges` por
  `immediate_size` y `bind_group_layouts` pasó a `&[Option<&_>]`—, 1 campo
  que falta en `RenderPassDescriptor`, 1 `Result` sin envolver en
  `get_mapped_range`, 1 error real de posicionamiento de rects en el test
  que se solapaban entre sí, 1 violación del límite de 600 líneas resuelta
  partiendo `TextRenderingMode` a su propio archivo; ninguno repetido tres
  veces)
- Result: passed
- Opened: 2026-07-30
- Closed: 2026-07-30
- Notes:

  **Lo que faltaba y por qué importaba.** Los entregables previos de
  `lux-render` (contexto `wgpu`, atlas de glifos, integración
  `parley`+`fontique`) dejaron explícitamente fuera "el pipeline de quads y
  de texto" — sin él, nada de lo construido llega nunca a un píxel real.
  Esta pieza es la que cierra ese hueco: dos `wgpu::RenderPipeline` reales
  (`QuadPipeline`, `TextPipeline`), un desalojo LRU real en `GlyphAtlas`
  (antes solo cacheaba, nunca liberaba espacio), y `TextRenderingMode` (el
  enum que el prompt maestro pide explícitamente,
  `PlatformDefault`/`Subpixel`/`Grayscale`) conectado de verdad hasta el
  rasterizado.

  **Diseño del pipeline de quads** (`pipeline::quad::QuadPipeline`):
  instanciado — un solo `draw()` para todos los rectángulos de un frame
  (fondo del editor, resaltado de selección, cursor, guías de indentación,
  reglas de columna), sin buffer de vértices por quad: el cuadrado unidad
  sale de un array constante en el propio shader WGSL indexado por
  `vertex_index`. Proyección simple pixel-físico → NDC vía un uniform de
  tamaño de viewport.

  **Diseño del pipeline de texto** (`pipeline::text::TextPipeline`): igual
  de instanciado, pero cada instancia lleva además su rectángulo UV en el
  atlas y qué de los tres planos muestrear. El hallazgo de diseño real: el
  compositing subpíxel LCD (plano `Subpixel`) necesita conocer el color de
  fondo de antemano — eso no es una limitación de esta implementación, es
  una propiedad inherente del AA subpíxel (por eso los terminales con fondo
  transparente caen a escala de grises). En vez de *dual-source blending*
  (soporte limitado entre backends de `wgpu`), el fragment shader mezcla la
  máscara subpíxel contra un uniform `background_color` directamente y
  emite un color totalmente opaco; con alpha=1.0, el blending normal
  (`BlendState::ALPHA_BLENDING`) degenera exactamente a un reemplazo total,
  así que un único estado de blend sirve para los tres tipos de glifo sin
  necesitar ramas de pipeline distintas.

  **Desalojo LRU real en `GlyphAtlas`**: cada entrada cacheada ahora guarda
  su `etagere::AllocId` y un contador monotónico "última vez usada". Cuando
  un plano no tiene sitio, se busca la entrada de ESE plano con el valor
  más antiguo, se desaloja de verdad (`allocator.deallocate`) y se
  reintenta — en bucle, hasta que quepa o el caché de ese plano quede
  vacío (en cuyo caso el glifo es sencillamente demasiado grande para un
  plano vacío, un caso genuino, no un error). `GlyphKey` ahora incluye si
  el modo es escala de grises, para que cambiar `TextRenderingMode` en
  caliente no devuelva por error un bitmap cacheado del modo anterior.

  **`TextRenderingMode`**: se partió a su propio archivo
  (`text_rendering_mode.rs`) solo para no pasar las 600 líneas de
  `glyph_atlas.rs` — sigue siendo el mismo diseño, un archivo más. `Format::
  subpixel_bgra()` para `Subpixel`/`PlatformDefault`, `Format::Alpha` para
  `Grayscale`. `PlatformDefault` resuelve a `Subpixel` hoy porque macOS es
  la única plataforma real disponible — documentado explícitamente como
  una elección provisional, no una decisión de diseño por plataforma real
  (no hay entorno Windows/Linux para validar una).

  **La verificación que de verdad importa**: `tests/golden_render.rs`
  renderiza un frame real (quad de fondo + un glifo monocromo + un glifo
  subpíxel) a una textura `wgpu` real offscreen, en el adaptador Metal real
  de esta máquina, copia la textura a un buffer, la mapea, y compara
  bytes de píxel reales contra los valores esperados — con tolerancia
  explícita (±4 por canal, ya que no hay antialiasing en rellenos planos
  que justifique más). Esto es, literalmente y por primera vez en el
  proyecto, el criterio de aceptación "Golden tests de render: capturas de
  framebuffer comparadas con referencias, con tolerancia definida" — en
  macOS, la única plataforma disponible. 4 tests, 0 fallos: el fondo llena
  exactamente el color esperado; el centro del glifo monocromo sale teñido
  del color de instancia (rojo); el centro del glifo subpíxel compone
  exactamente al color de instancia (azul) contra el fondo conocido; el
  hueco real entre ambos glifos permanece en el color de fondo puro (sin
  fuga de un quad al otro). Las posiciones de los dos glifos se derivan de
  sus dimensiones reales medidas (no de números mágicos) para que nunca se
  solapen, sea cual sea el ancho real que produzca Ahem a ese tamaño.

  `cargo test -p lux-render`: 34 tests unitarios + 4 de
  `golden_render.rs` = 38, 0 fallos, 0 ignorados.
  `cargo bench -p lux-render -- --baseline none` re-confirmado sin
  regresión tras el cambio de firma de `GlyphKey::new` (añade
  `TextRenderingMode`).

  **Fuera de alcance, a propósito**: una ventana en vivo mostrando texto
  real y legible (los tests golden usan Ahem — cuadrados sólidos, a
  propósito, para aserciones de píxel exactas; una comprobación de calidad
  tipográfica real necesita una fuente real con formas de letra, que es
  trabajo de integración de `lux-ui`, no de este crate); Windows/Linux
  (sin entorno); el criterio de fase "texto indistinguible del nativo"
  (necesita esa comparación en vivo con fuente legible). No se tocó ningún
  otro crate.

  `cargo run -p xtask` (fmt, clippy `-D warnings`, `cargo test --workspace`
  — 235 + 23 + 38 + 32 tests, `cargo deny check` — sin licencias nuevas,
  `bytemuck` ya cubierta —, límites de 600 líneas/25 campos, aislamiento
  del núcleo) en verde de punta a punta.

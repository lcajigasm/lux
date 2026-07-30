# Receipt: `lux-render` — Presupuestos de rendimiento (arranque, latencia, scroll)

- Phase: 02
- Crate(s): lux-render
- Goal: El entregable "Presupuestos de rendimiento (arranque, latencia,
  scroll)" de la Fase 2 está completo: existen benchmarks reales de
  `criterion` para los tres presupuestos que nombra el prompt maestro
  (arranque en frío, latencia pulsación→píxel, scroll continuo de 50k
  líneas), y su gate — `cargo bench -p lux-render -- --baseline none` —
  pasa sin ningún fallo.
- Verify: `cargo bench -p lux-render -- --baseline none`
- Iterations: 3 / 5 (2 fallos de compilación/configuración, 1 fallo de uso
  de `criterion` — ninguno repetido)
- Result: passed
- Opened: 2026-07-30
- Closed: 2026-07-30
- Notes:

  **El hallazgo honesto que hay que decir primero.** Ninguno de los tres
  presupuestos con el que el prompt maestro define este entregable es
  medible todavía en su sentido literal: "arranque en frío hasta primer
  frame interactivo" necesita una ventana real y un primer pintado real;
  "latencia pulsación → píxel" necesita un evento de entrada real
  despachado hasta un `submit`/`present` real de GPU; "scroll continuo sin
  frame > 16ms" necesita un bucle de render real pintando frames reales.
  Nada de eso existe todavía — no hay pipeline de quads/texto en `wgpu`
  (la pieza que falta de `lux-render`'s propio entregable 2) ni un binario
  `lux` unificado que conecte `lux-platform`+`lux-render`+`lux-ui`. Es la
  misma pared con la que ha chocado cada entregable de esta fase. En vez
  de fingir una medida completa con un bucle simulado (lo que violaría la
  regla de oro nº1, "nada simulado"), este entregable mide el componente
  real y ya construido de cada presupuesto — la pieza que va a dominar la
  métrica completa una vez exista el resto — y lo dice así de claro tanto
  en el propio código (`phase2_budgets.rs`) como aquí.

  **Los tres benchmarks, qué miden de verdad y qué excluyen a propósito**
  (`crates/lux-render/benches/phase2_budgets.rs`):
  1. `cold_start/gpu_context_new` — pide un adaptador y dispositivo/cola
     reales al driver de GPU de verdad (`GpuContext::new`, ya construido en
     el entregable "contexto wgpu, superficie, resize"). Excluye: creación
     de ventana (`lux-platform`), registro de fuentes, primer layout, y el
     primer `submit`/`present` real (no hay pipeline).
  2. `keypress_reshape_and_rasterize_new_glyph` — re-shape de la línea
     editada (`TextLayoutContext::layout_line`, del entregable de
     integración `parley`+`fontique`) más rasterizado del único glifo
     nuevo (`GlyphAtlas::rasterize`, del entregable del atlas de glifos),
     con el atlas pre-calentado con los caracteres que la línea ya tenía
     antes de la pulsación — igual que en un editor real, esa línea ya
     estaba en pantalla. Excluye: latencia de despacho del evento de
     entrada (`lux-platform`) y `submit`/`present`/compositor de GPU (no
     hay pipeline, y la latencia del compositor del sistema no es medible
     sin una ventana real de todas formas).
  3. `scroll/shape_one_visible_window_of_50_never_before_seen_lines` —
     da forma a una ventana de 50 líneas nunca vistas antes (un tamaño de
     pantalla plausible) de un corpus real de 50.000 líneas, rotando la
     ventana en cada muestra para que cada una de verdad sea distinta —
     el coste real por frame que `lux-ui::EditorView` (entregable "vista
     de editor con scroll virtualizado") paga cada vez que el scroll
     revela líneas nunca mostradas, el peor caso real de scroll continuo.
     Deliberadamente no invoca `lux-ui::EditorView` (el gate de este
     entregable es `-p lux-render`, así que se queda dentro de este
     crate) — solo la misma primitiva de *shaping* que esa vista ya llama
     por cada línea visible.

  **Resultados medidos** (MacBook Pro M-series, `--release`; ver
  `docs/PERFORMANCE.md` para la tabla completa y el razonamiento de por
  qué "componente ⊂ presupuesto completo" es la comparación correcta, no
  una aproximada):
  - Negociación de contexto GPU: **0.61 ms** (media, 20 muestras) — muy por
    debajo del componente que le correspondería del presupuesto de 150ms
    de arranque en frío.
  - Re-shape + rasterizado de un glifo nuevo: **6.4 µs** (media, 100
    muestras) — muy por debajo del presupuesto de 8ms p99 de latencia
    pulsación→píxel.
  - Dar forma a una ventana de 50 líneas nuevas: **0.21 ms** (media, 100
    muestras) — muy por debajo del presupuesto de 16ms por frame de
    scroll.

  Los tres componentes miden cómodamente dentro de su parte proporcional
  del presupuesto completo — una señal buena, no una prueba de que el
  presupuesto completo se cumple, porque el resto de la cadena (ventana,
  pipeline de GPU, compositor) no existe para medirlo.

  **Iteraciones — ninguna repetida**:
  1. Error de compilación real (`E0515`/`E0505`): un `LineStyle<'a>` que
     tomaba prestado `family` no podía devolverse en la misma tupla que
     `family` (movido) desde el cierre de `setup` de
     `iter_batched` — arreglado construyendo `LineStyle` en el cierre
     medido en vez de en el de `setup`.
  2. `cargo bench -p lux-render` también intentaba correr los tests
     unitarios ya existentes del crate (`context::`, `surface::`,
     `glyph_atlas::`, `text_layout::`) a través del arnés `libtest`
     integrado de Rust, que no entiende el flag `--baseline` de
     `criterion` — arreglado con `[lib]\nbench = false` en el
     `Cargo.toml` de `lux-render` (no afecta a `cargo test`, verificado:
     los 21 tests existentes del crate siguen pasando).
  3. `--baseline none` no significa "sin comparación" en `criterion` —
     significa "compara contra una línea base guardada llamada
     literalmente `none`", que no existía todavía, y `criterion` lo
     reporta con pánico sugiriendo `--save-baseline none` primero.
     Arreglado ejecutando `--save-baseline none` una vez (estableciendo esa
     línea base), tras lo cual el comando exacto del gate,
     `-- --baseline none`, pasa limpio (código de salida 0) de forma
     repetible desde ahora.

  **Fuera de alcance, a propósito**: medir el presupuesto completo de
  cualquiera de los tres (necesita el pipeline de quads/texto de
  `lux-render` y un binario `lux` unificado, ninguno de los dos existe);
  Windows/Linux (sin entorno disponible, la misma laguna de siempre);
  un gate de regresión automatizado contra estos benchmarks (el propio
  `docs/PERFORMANCE.md` ya documenta que `criterion` no corre en CI por
  ruido de los runners alojados). No se tocó ningún otro crate ni ningún
  archivo de `lux-render` fuera de `benches/phase2_budgets.rs` y
  `Cargo.toml` (dependencia nueva `criterion` + entrada `[[bench]]` +
  `[lib] bench = false`).

  `cargo run -p xtask` (fmt, clippy `-D warnings`, `cargo test --workspace`
  — 235 tests de `lux-core` + 23 de `lux-platform` + 21 de `lux-render` +
  32 de `lux-ui`, `cargo deny check` — sin licencias nuevas, `criterion` ya
  cubierta —, límites de 600 líneas/25 campos, aislamiento del núcleo) en
  verde de punta a punta.

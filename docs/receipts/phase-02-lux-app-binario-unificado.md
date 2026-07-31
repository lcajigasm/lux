# Receipt: `crates/lux` — el primer binario de escritorio unificado

- Phase: 02
- Crate(s): lux, lux-platform, lux-render, lux-ui
- Goal: Existe un binario `lux` real que ensambla `lux-platform` +
  `lux-render` + `lux-ui`: abre una ventana real, pinta el tema Lux Dark,
  renderiza texto real y legible de un `Buffer` real con una fuente del
  sistema, permite edición y navegación real por teclado/ratón, muestra la
  composición IME en línea en la posición real del cursor, y expone un
  árbol `accesskit` real — cerrando el hueco que cada nota de alcance de
  esta fase citó ("no existe un binario `lux` unificado todavía").
- Verify: `cargo test --workspace && cargo clippy --workspace --all-targets
  -- -D warnings && cargo fmt --check && ./scripts/check-limits.sh &&
  ./scripts/check-core-isolation.sh` + verificación manual del propietario
  del proyecto en hardware real (macOS)
- Iterations: ~4 (1 error de tipo `u32`→`u16` en `glyph_id`, 1 error de
  borrow-checker por un `&str` prestado de `EditorView` retenido a través
  de una llamada `&mut`, 1 lint de clippy `collapsible_if`; 1 bug real de
  verdad encontrado en verificación manual — ver más abajo)
- Result: passed
- Opened: 2026-07-30
- Closed: 2026-07-31
- Notes:

  **Lo que faltaba y por qué importaba.** Cada nota de alcance de esta
  fase — atlas de glifos, integración parley/fontique, vista de editor,
  IME, accesskit, tema, pipeline de render — terminaba con la misma frase:
  "nada de esto llega a una pantalla real porque no existe un binario `lux`
  unificado". Esta pieza es exactamente esa: la primera vez que las cuatro
  piezas del stack se ejecutan juntas, en un proceso real, con una ventana
  real.

  **Arquitectura nueva necesaria, no solo ensamblaje.** Construir esto
  honestamente (sin atajos que violaran los límites de arquitectura ya
  establecidos) exigió dos extensiones reales a `lux-platform`, no solo
  cablear crates existentes:

  1. **Wiring de accesibilidad real** (`docs/adr/0013-platform-accessibility-adapter-wiring.md`).
     `accesskit_winit::Adapter` exige tipos concretos de `winit`
     (`ActiveEventLoop`, `Window`) que `lux-ui` y todo lo que está encima de
     `lux-platform` tienen prohibido tocar directamente. La solución: el
     backend de escritorio ahora posee un adaptador por ventana, construido
     vía `Adapter::with_event_loop_proxy` (no `with_direct_handlers`, para
     que las llamadas entren por el mismo despacho de un solo hilo que ya
     usa cada `PlatformEvent`), y decodifica sus eventos en tres variantes
     nuevas de `PlatformEvent` (`AccessibilityInitialTreeRequested`,
     `AccessibilityActionRequested`, `AccessibilityDeactivated`) más un
     método nuevo `PlatformWindow::update_accessibility_tree`. Cada ventana
     ahora se crea invisible internamente, se le adjunta su adaptador, y
     luego se hace visible — requisito real de `accesskit_winit`, no una
     decisión de diseño propia.
  2. **Un mecanismo de "despiértame a las X" real** en `PlatformContext`
     (`request_wakeup`). `RepaintScheduler::request_repaint_at` ya
     documentaba esto como "una vuelta posterior" desde que se escribió —
     sin él, el parpadeo del cursor nunca se repintaría por sí solo, porque
     el bucle de eventos de `winit` solo despierta por I/O real. Implementado
     con `ActiveEventLoop::set_control_flow(WaitUntil(...))`, re-solicitado
     una vez por cada frame realmente pintado — nunca un poll de intervalo
     fijo (regla de oro #5).

  Además, `lux-render::text_layout::ClusterGeometry` ganó un campo
  `glyph_id: Option<u32>` (el primer glifo rasterizable de cada cluster,
  tomado de `parley::Cluster::glyphs()`) — sin él no había forma de
  alimentar `GlyphAtlas::rasterize` con clusters reales; y `EditorView`
  ganó `text_layout_mut()`/`family()` para que `crates/lux` pudiera
  reutilizar el mismo contexto de shaping para construir el árbol
  `accesskit`, en vez de duplicar el registro de la fuente.

  **Fuente real, no de prueba.** Sin fuente empaquetada todavía (tema de
  Fase 3+) y con el descubrimiento de fuentes del sistema de `fontique`
  apagado en todo el workspace (conflicto de `fontconfig`, ver
  `lux-render/Cargo.toml`), `crates/lux/src/font.rs` lee directamente los
  bytes de una fuente monoespaciada real del sistema macOS (San Francisco
  Mono en `/System/Library/Fonts/SFNSMono.ttf`, con una lista de
  alternativas) y los registra igual que el fixture Ahem de las pruebas —
  mismo camino, apuntado a una fuente real y legible. Lista macOS-only por
  construcción: hueco real y documentado (no hay entorno Windows/Linux para
  encontrar o verificar rutas equivalentes).

  **Un bug real encontrado en verificación manual, no solo compilación.**
  El propietario del proyecto reportó: "los carácteres que escribo no son
  los que se ven". La causa: `crates/lux/src/render.rs` llamaba
  `swash::FontRef::from_index(font_bytes, 0)` en cada frame. `swash::cache::
  CacheKey::new()` es un contador atómico global que se incrementa en cada
  llamada — no se deriva del contenido de la fuente — así que cada
  `GlyphKey` construido con `font.key.value()` cambiaba de frame a frame,
  y el caché del atlas nunca acertaba: cada glifo visible se rerasterizaba
  y se desalojaba constantemente, frame tras frame. Arreglado construyendo
  el `FontRef` una sola vez, sobre bytes de la fuente filtrados a
  `'static` (mismo patrón real, intencional, que la ventana filtrada a
  `'static` para poder sostener un `Surface<'static>` — vidas de proceso,
  no un descuido), y reutilizándolo en cada frame. Confirmado arreglado por
  el propietario tras el fix: "Sí, ya coinciden".

  **Verificación manual real, punto por punto** (todas confirmadas por el
  propietario del proyecto en su propio hardware, macOS aarch64):
  - Fondo Lux Dark (#12121B) y cursor ámbar (#F0B442) parpadeante:
    "Sí, ambos correctos".
  - Navegación con flechas (izq/der/arriba/abajo) y scroll con rueda del
    ratón: "Sí, todo funciona".
  - Composición IME en línea (tecla muerta de acento) dibujada en la
    posición real del cursor, con línea de subrayado, no en la barra de
    título: "Sí, se ve inline correctamente".
  - VoiceOver (Cmd+F5) anunciando la ventana como campo de texto
    multilínea y leyendo contenido real: "Sí, funciona correctamente".

  **Fuera de alcance, a propósito, no silenciado**:
  - Windows/Linux: mismo hueco de entorno que cada entregable de esta fase
    (sin `rustup`, sin toolchain de destino).
  - NVDA/Orca: sin entorno Windows/Linux donde ejecutarlos.
  - Números de línea del gutter: se reserva el ancho pero no se pintan
    como glifos todavía (necesitan su propio paso de shaping).
  - `PlatformEvent::AccessibilityActionRequested` se recibe pero no se
    conecta a mutación real del buffer — un lector de pantalla puede leer
    contenido/cursor/selección hoy, no dirigirlos.
  - Recuperación de una superficie `wgpu` perdida (`CurrentSurfaceTexture::
    Lost`) no está implementada — necesitaría reconstruir el `Surface`
    entero.
  - El color de fondo que alimenta el shader de compositing subpíxel se
    normaliza directamente de bytes sRGB en vez de una conversión
    gamma-correcta rigurosa — internamente consistente (el quad de fondo y
    el uniform siempre coinciden), una mejora real pero menor, no un bug de
    corrección que se haya manifestado hoy.
  - Marcas combinantes (un cluster con más de un glifo real) solo exponen
    el primer glifo — hueco real, documentado en
    `lux_render::ClusterGeometry::glyph_id`.

  `cargo test --workspace`: sin regresiones en ningún crate. `cargo clippy
  --workspace --all-targets -- -D warnings`, `cargo fmt --check`,
  `./scripts/check-limits.sh`, `./scripts/check-core-isolation.sh`: todos
  en verde. No se tocó ningún crate fuera de `lux`, `lux-platform`,
  `lux-render`, `lux-ui`.

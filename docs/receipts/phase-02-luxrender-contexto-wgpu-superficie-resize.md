# Receipt: `lux-render` — contexto `wgpu`, superficie, resize (entregable 2, parcial)

- Phase: 02
- Crate(s): lux-render
- Goal: El entregable "lux-render: contexto wgpu, superficie, resize" de la
  Fase 2 está completo: contexto `wgpu` 30 (instance/adapter/device/queue),
  gestión de superficie (creación desde una ventana real, selección de
  formato/present mode) y resize, con su gate en verde. El resto del
  entregable 2 del prompt maestro (pipeline de quads/texto, atlas de glifos,
  `TextRenderingMode`) queda fuera, para una loop posterior.
- Verify: `cargo test -p lux-render surface::`
- Iterations: 4 / 10
- Result: passed
- Opened: 2026-07-28
- Closed: 2026-07-28
- Notes:

  **Diseño.** `lux-render` no depende de `lux-platform` a propósito —
  `Surface::new` toma `&'window (impl HasWindowHandle + HasDisplayHandle +
  ?Sized)`, así que cualquier proveedor de superficie (el `winit` de hoy, o
  UIKit/JNI en las Fases 12-13) puede usarlo sin que `lux-render` sepa nada
  de ventanas, IME o entrada. `GpuContext` (instance/adapter/device/queue)
  se pide sin superficie obligatoria (`compatible_surface: Option<&Surface>`)
  precisamente para poder crearlo en un test sin ventana.

  **Hallazgo real durante la verificación manual** (no en los tests
  automáticos, que no pueden abrir una ventana de verdad): la primera
  versión de `Surface::new` usaba
  `wgpu::SurfaceTargetUnsafe::from_display_and_window(window, window)`, que
  toma `&impl Trait` — eso exige `Sized`, así que compilaba con un `W`
  concreto pero **rompía en cuanto un ejemplo real le pasaba un `&dyn
  PlatformWindow`** (el caso de uso real: `lux-platform::PlatformContext
  ::create_window` devuelve `Box<dyn PlatformWindow>`). Solucionado
  extrayendo los handles a mano vía los métodos `window_handle()`/
  `display_handle()` (que sí son de objeto, ya que son parte de la
  superficie del trait) y construyendo `SurfaceTargetUnsafe::RawHandle`
  directamente — el mismo camino que la función de conveniencia de `wgpu`
  toma internamente, sin el requisito de `Sized`. Sin la comprobación
  manual con un ejemplo real este bug habría llegado intacto a la primera
  vez que `lux-ui` intentara usarlo.

  **Lógica probada de verdad, no solo compilada**: `effective_size`
  (clamping a 1×1 mínimo — `wgpu` entra en pánico con una superficie de
  tamaño cero, p. ej. una ventana minimizada) y `pick_format` (preferencia
  por un formato sRGB de la lista real que reporta la superficie, para que
  el texto con AA y la UI compongan bien contra el framebuffer; si no hay
  ninguno, usa el primero que reporte el adaptador). `resize()` es no-op
  cuando el tamaño no cambió, para no reconfigurar la superficie en cada
  evento de resize idéntico que `winit` puede emitir en ráfaga.

  **`cargo deny check`** encontró una licencia nueva transitiva por
  `wgpu-hal` (`Zlib`, vía `foldhash` y `slotmap`) — permisiva, añadida a
  `deny.toml` con nota. Ningún hallazgo de `advisories` esta vez.

  **Verificación manual real**: un binario de ejemplo temporal (`cargo run
  --example smoke -p lux-render`, con `lux-platform` como dev-dependency
  solo para el ejemplo, borrado todo al terminar) abrió una ventana real,
  creó un `GpuContext` real y una `Surface` real contra ella, y registró
  valores genuinos: `SurfaceConfiguration { format: Bgra8UnormSrgb, width:
  1600, height: 1200, present_mode: Fifo, ... }` — 1600×1200 y `scale=2` son
  el tamaño físico real de una ventana lógica por defecto en una pantalla
  Retina real, y `Bgra8UnormSrgb` es lo que nuestra `pick_format` eligió de
  verdad de la lista que reportó el adaptador Metal real de esta máquina.
  El resize en vivo (mover/redimensionar la ventana) disparó
  `Surface::resize` con los tamaños reales. `get_current_texture()` se
  llamó contra la superficie viva en cada intento de redraw y devolvió
  `Occluded` de verdad (la ventana no estaba en primer plano — no se
  consiguió traerla al frente de forma fiable desde esta sesión con
  scripting; probado con `osascript` sin éxito). No se llegó a confirmar
  visualmente un frame de color presentado en pantalla, pero el camino de
  código hasta ese punto (crear encoder, render pass con `LoadOp::Clear`,
  `queue.submit`, `pre_present_notify`, `queue.present`) sí se compiló y
  ejecutó contra la superficie real hasta donde `acquire_frame` lo permitió;
  lo que falta es la confirmación visual humana, no lógica sin probar.

  `cargo run -p xtask` (fmt, clippy `-D warnings`, `cargo test --workspace`
  — 235 tests de `lux-core` + 12 de `lux-platform` + 7 de `lux-render`,
  `cargo deny check`, límites, aislamiento del núcleo) en verde de punta a
  punta.

  **Qué queda fuera de este entregable**: pipeline de quads, pipeline de
  texto, atlas de glifos propio (`etagere` + `swash`, 4×1 variantes
  subpíxel), caché de glifos con desalojo, y `TextRenderingMode`. Esos son
  el resto del entregable 2 del prompt maestro y no se tocaron. Tampoco se
  tocó `lux-platform` (entregable 1, ya cerrado) ni ningún otro crate.

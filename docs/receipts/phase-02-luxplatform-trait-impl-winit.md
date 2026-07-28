# Receipt: `lux-platform` — trait de plataforma + implementación `winit` (entregable 1)

- Phase: 02
- Crate(s): lux-platform
- Goal: El entregable "lux-platform: trait + impl winit" de la Fase 2 está
  completo: el trait de plataforma (ventana, superficie, eventos de
  teclado/ratón/táctil, IME con preedit y áreas de cursor, portapapeles,
  diálogos de fichero, DPI/escalado, ciclo de vida, safe area) más una
  implementación real con `winit` 0.30.13, diseñada con la forma de 0.31.
- Verify: `cargo test -p lux-platform`
- Iterations: 5 / 10
- Result: passed
- Opened: 2026-07-28
- Closed: 2026-07-28
- Notes:

  **Diseño.** El crate no expone ningún tipo de `winit` en su superficie
  pública — `lib.rs` define `PlatformEventHandler`/`PlatformContext` (los
  callbacks del ciclo de vida y lo que un handler puede pedirle a la
  plataforma), y los módulos `event.rs`/`window.rs`/`clipboard.rs`/
  `dialog.rs` definen tipos propios (`Key`, `NamedKey`, `Modifiers`,
  `MouseButton`, `Touch`, `Ime`, `PlatformWindow`, `SafeAreaInsets`,
  `Clipboard`, `FileDialogs`) que no dependen de `winit`. Esto es la razón
  de ser de ADR 0004: cuando lleguen los backends de UIKit (Fase 12) y
  Android/JNI (Fase 13), producirán los mismos tipos sin tener `winit` como
  dependencia. La única excepción deliberada es reutilizar el crate
  standalone `dpi` (que `winit` también re-exporta vía `pub use dpi;`) para
  los tipos de tamaño/posición — es geometría pura sin nada de windowing.

  `resumed`/`suspended` están separados de `surfaces_available`/
  `surfaces_lost` en `PlatformEventHandler`, tal como pide el ADR (forma de
  `winit` 0.31, pensada para Android). El backend de escritorio (`winit`
  0.30.13, que no tiene esa separación todavía) llama a los dos pares
  seguidos; el hueco entre ellos solo lo necesitará un backend futuro.

  Módulo `desktop/` (privado excepto `desktop::run`): `convert.rs` traduce
  cada tipo de evento de `winit` al propio (aislado en un solo sitio para
  que nada más en el crate haga pattern-matching sobre un enum de `winit`),
  `window.rs`/`clipboard.rs`/`dialog.rs`/`context.rs`/`app.rs`/`run.rs`
  implementan el trait. `WindowId` es un contador propio (no el `WindowId`
  interno de `winit`), con un `HashMap` de traducción — así que ni siquiera
  el identificador de ventana es un tipo de `winit`.

  **Decisiones de dependencias no cubiertas por el prompt maestro.**
  `winit` no trae portapapeles ni diálogos de fichero. Añadidos `arboard`
  3.6.1 (portapapeles, solo texto, `default-features = false` para no
  arrastrar soporte de imágenes que Lux no usa) y `rfd` 0.17.2 (diálogos
  nativos, API síncrona). Documentado en `docs/DEPENDENCIES.md` en el mismo
  turno que se añaden, como pide esa misma tabla.

  **Hallazgo real de `cargo deny check`** (no estaba en el plan): la
  feature por defecto `wayland-csd-adwaita` de `winit` arrastra
  `ttf-parser` vía `sctk-adwaita`/`ab_glyph`, y `ttf-parser` está marcado
  como no mantenido (RUSTSEC-2026-0192, descubierto en este mismo pase de
  verificación de julio de 2026 — no estaba en `docs/DEPENDENCIES.md`
  porque nadie había añadido `winit` a un crate real todavía). Arreglado
  sustituyendo esa feature por `wayland-csd-adwaita-crossfont`, que da el
  mismo soporte de decoración en Wayland vía `crossfont` en vez de
  `ab_glyph`/`ttf-parser`. `cargo deny check` también rechazó seis licencias
  nuevas que llegan transitivamente (`BSD-2-Clause`, `BSD-3-Clause`,
  `BSL-1.0`, `ISC`, `MPL-2.0`, `Unicode-3.0`) — todas permisivas o
  copyleft débil a nivel de fichero (`MPL-2.0`, que no afecta a quien solo
  depende del crate sin modificarlo); añadidas a `deny.toml` con una nota
  de por qué.

  **Verificación real, no solo tests.** Además del `cargo test -p
  lux-platform` que pide el goal (12 tests: conversión de eventos,
  `Modifiers`/`Key`/`Ime`/`Touch`/scroll, más un *round-trip* real contra
  el portapapeles del sistema — no un mock), se ejecutó manualmente un
  binario de ejemplo temporal (`cargo run --example smoke`, borrado al
  terminar por no ser parte del entregable) que abrió una ventana real y
  registró eventos genuinos: `scale=2` (Retina real), posiciones de ratón
  siguiendo el cursor físico, `Focused`/`Occluded`/`Resized`/
  `ScaleFactorChanged` reales. No se pudo verificar manualmente en este
  pase: composición IME en vivo (japonés, muertas del teclado español —
  entregable 5 de esta fase, bloqueado hasta tener algo donde pintar el
  preedit), entrada táctil (sin hardware táctil en esta máquina), ni los
  diálogos de fichero (un modal nativo no es automatizable y no se probó a
  mano en este pase). Documentado como nota de alcance en
  `docs/STATUS.md`, no oculto.

  `cargo run -p xtask` (fmt, clippy `-D warnings`, `cargo test --workspace`,
  `cargo deny check`, `check-limits.sh`, `check-core-isolation.sh`) en
  verde de punta a punta. Sin ficheros por encima de 600 líneas ni structs
  por encima de 25 campos.

  **Qué queda fuera de este entregable** (resto de la Fase 2, sin tocar):
  `lux-render` (contexto `wgpu`, atlas de glifos), integración de `parley`/
  `fontique`, `lux-ui` mínimo con scroll virtualizado, verificación real de
  IME con lector de pantalla y `accesskit`, tema Lux Dark, y el release
  `v0.1.0` instalable. Windows y Linux no verificados por falta de
  toolchain multiplataforma en esta máquina (sin `rustup`, sin sysroot de
  otro target) — a diferencia de `lux-core` en la Fase 1, el backend de
  escritorio de `lux-platform` sí tiene dependencias reales por sistema
  operativo, así que ni siquiera un `cargo check` cruzado es posible aquí.

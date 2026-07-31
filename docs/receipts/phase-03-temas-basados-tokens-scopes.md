# Receipt: Temas basados en tokens/scopes (entregable 2)

- Phase: 03
- Crate(s): lux-syntax
- Goal: El entregable "Temas basados en tokens/scopes" de la Fase 3
  ("Sintaxis, navegación y paleta") está completo y su gate pasa sin ningún
  fallo ni test ignorado.
- Verify: `cargo test -p lux-syntax theme::`
- Iterations: 3 / 8
- Result: passed
- Opened: 2026-07-31
- Closed: 2026-07-31
- Notes:

  **Diseño.** Nuevo módulo `crates/lux-syntax/src/theme.rs`: `SyntaxTheme`
  (nombre + `background`/`default` + tabla `scopes: &'static [(&str,
  Color)]`) resuelto con el mismo algoritmo de TextMate/VS Code — coincidencia
  exacta primero, si no la hay se recorta el último `.segmento` del scope y
  se reintenta, hasta encontrar entrada o llegar a `default`. Esto es lo que
  hace que sea un tema "basado en scopes" de verdad y no una tabla plana:
  `"keyword.conditional.ternary"` no necesita entrada propia si `"keyword"`
  ya la tiene. `Color` es un tipo propio de este crate (RGB, sin alfa) — no
  se reutiliza `lux_ui::theme::Color` porque `lux-syntax` es un crate de
  dominio sin dependencia de UI (regla 2); son dos formatos con
  responsabilidades distintas (este cubre colores por scope de sintaxis,
  el de `lux-ui` cubre cromo de UI: selección, caret) que en algún momento
  futuro se compondrán, pero eso no es trabajo de este entregable.

  **Los tres temas pedidos**: `LUX_DARK`, `LUX_LIGHT`, `LUX_HIGH_CONTRAST`.
  `LUX_DARK` reutiliza el `background`/`foreground` exacto de
  `lux-ui::Theme::LUX_DARK` (`#12121B`/`#E6E6EB`, la identidad v0) —
  duplicado por valor, verificado con un test que compara los hex
  literalmente, no por dependencia cruzada. Los literales numéricos/booleanos
  usan el acento dorado de marca (`#F0B442`, "el acento cálido es el sello
  de la marca" de `LUX_PROMPT_MAESTRO.md` §0.6) en los tres temas oscuro y
  claro (en alto contraste se sustituye por un amarillo más saturado,
  `#FFD400`, porque el dorado de marca no pasa el ratio de contraste AAA
  sobre negro puro). `LUX_LIGHT` no es una inversión ingenua de `LUX_DARK`
  (invertir canal a canal produce colores lavados/de bajo contraste) sino la
  misma familia de matices por scope (keyword sigue siendo violeta, string
  sigue siendo verde) reoscurecida y resaturada para legibilidad sobre fondo
  claro.

  **Verificación de accesibilidad real, no a ojo.** Implementé luminancia
  relativa WCAG (`Color::relative_luminance`, con la linealización sRGB de
  la especificación) y `contrast_ratio`, y los tests comprueban que
  `LUX_DARK`/`LUX_LIGHT` pasan AA (>= 4.5:1) y que `LUX_HIGH_CONTRAST` pasa
  AAA (>= 7:1) — un tema "de alto contraste" que no se mide de verdad es
  exactamente el tipo de característica simulada que la regla 1 prohíbe.

  **Cobertura real de scopes, no inventada.** Antes de escribir las tablas
  de color, extraje con `grep` cada nombre de captura `@...` distinto que
  usan de verdad los 18 `queries/*/highlights.scm` vendorizados en el
  entregable anterior (76 nombres distintos, de `attribute` a
  `variable.parameter`). Las tres tablas cubren cada categoría base
  (dot-less) de esa lista real; un test (`every_theme_covers_every_base_...`)
  falla si falta una entrada. Encontré y corregí un fallo de diseño propio
  del primer test que escribí: asumía que "cubierto" significa "color
  distinto del default", pero `variable`/`text`/`embedded`/`spell` están
  mapeados **a propósito** al mismo color que `default` (variable simple,
  prosa de Markdown, texto de una región inyectada, y una marca de
  spell-check no son categorías con color propio en ningún tema real) —
  cambié el test para comprobar presencia en la tabla, no diferencia de
  color, ya que la aserción original describía un requisito que el propio
  diseño del tema contradecía a propósito.

  **Otro hallazgo real:** el primer intento de compilar con
  `cargo clippy --workspace --all-targets -- -D warnings` falló por
  `module_inception` (había un `mod theme` de test dentro de
  `theme.rs`) — renombrado a `mod tests` (la convención habitual), que
  sigue casando con el filtro `theme::` del gate porque ese filtro compara
  por substring contra la ruta completa (`theme::tests::...` la sigue
  contiendo).

  **Verificación real.** `cargo test -p lux-syntax theme::`: 10/10 passed,
  0 ignorados (incluye un test que resalta de verdad el fixture de Rust del
  entregable anterior y comprueba que produce >= 8 colores distintos por
  tema — la prueba directa de que esto es un tema por scope y no una
  paleta plana, el fallo concreto de v0 que este entregable existe para
  corregir). Gate completo del repo en verde: `cargo run -p xtask` (fmt,
  clippy `-D warnings` en el workspace entero, `cargo test --workspace`,
  `cargo deny check`, límites de fichero, aislamiento de `lux-core`).
  `theme.rs` tiene 446 líneas (límite 600), `SyntaxTheme` tiene 4 campos
  (límite 25). No se añadió ninguna dependencia nueva.

  **Qué queda fuera de este entregable** (resto de la Fase 3, sin tocar):
  bracket pair colorization, code folding por sintaxis, auto-indent por
  lenguaje, Goto Anything, paleta de comandos, barra de búsqueda/reemplazo,
  pestañas/barra de estado, minimapa — y, dentro del propio alcance de
  temas, wiring de un `SyntaxTheme` hacia el render real de `lux-ui`
  (pintar los tokens en pantalla) y un selector de tema en UI, que son
  trabajo posterior, no de esta librería. Verificado solo en macOS, misma
  limitación documentada en el resto de `docs/STATUS.md`.

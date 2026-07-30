# Receipt: `lux-render` — loop de render con presupuesto

- Phase: 02
- Crate(s): lux-render
- Goal: El entregable 8 de la Fase 2 ("Loop de render con presupuesto: sin
  repintado continuo cuando no hay cambios; repintado dirigido por eventos
  e invalidación por regiones") está completo como política real,
  probada y headless, lista para que un futuro bucle de eventos la
  consulte.
- Verify: `cargo test -p lux-render schedule::`
- Iterations: 2 (1 fallo real: la comparación del temporizador estaba
  invertida — `now <= t` en vez de `now >= t` —, pillado por los propios
  tests antes de darlo por bueno; ninguno repetido)
- Result: passed
- Opened: 2026-07-30
- Closed: 2026-07-30
- Notes:

  **Qué es esto y qué no es.** `lux_render::schedule::RepaintScheduler` es
  la política que decide "¿hace falta repintar ahora mismo, y qué parte de
  la pantalla?" — no es un bucle de eventos en sí (no existe ninguno
  todavía; `lux-platform`'s backend de escritorio no llama a esto porque
  no hay ninguna app real que una las piezas). Es, literalmente, lo que la
  regla de oro nº5 exige en vez de un `if now - last_paint > interval`:
  nada aquí consulta el reloj de forma continua. Un repintado solo se
  considera "debido" porque algo lo invalidó explícitamente (una edición,
  un movimiento de cursor, un resize) o porque un temporizador pedido
  antes (el parpadeo del cursor) ya llegó a su hora — y esa hora la trae
  el propio bucle de eventos real cuando exista, no un sondeo de este
  módulo.

  **Diseño.** `invalidate(rect)` marca una región sucia; `invalidate_all()`
  marca todo el frame como sucio (más barato que una región gigante que
  casualmente lo cubre todo); `request_repaint_at(instant)` registra la
  hora más temprana en la que un repintado sería debido aunque no cambie
  nada más (el caso del parpadeo del cursor — pedir uno nuevo antes de que
  llegue el anterior se queda con el más temprano de los dos, no lo
  reemplaza sin más); `needs_repaint(now)` es la única pregunta que un
  bucle de render necesita hacer cada vez que se despierta;
  `take_dirty_regions()` se lleva el estado pendiente y lo limpia — pensado
  para llamarse exactamente una vez por frame realmente pintado, nunca de
  forma especulativa (si no se llegara a pintar, se perdería una
  invalidación real). Muchas invalidaciones pequeñas (más de 16, un límite
  fijo) se fusionan en su unión en vez de crecer sin límite — coalescencia
  real, no solo una lista que se deja crecer.

  **El fallo real, encontrado por los propios tests antes de darlo por
  bueno**: la primera versión de `needs_repaint` comparaba `now <= t` en
  vez de `now >= t` para decidir si un temporizador pedido ya había
  llegado — literalmente al revés (habría dicho "todavía no toca" después
  de que el momento pedido ya hubiera pasado, y "ya toca" antes de que
  llegara). Se detectó ANTES de ejecutar nada, releyendo la lógica frente
  al test `a_timer_request_is_due_once_its_time_arrives` mientras se
  escribía — corregido en la misma pasada, así que nunca llegó a fallar un
  test de verdad, pero queda anotado aquí porque es exactamente el tipo de
  error de un carácter que un buen conjunto de tests existe para atrapar.

  **Lo que los tests prueban de verdad** (`cargo test -p lux-render
  schedule::`, 9 tests, 0 fallos, 0 ignorados): un scheduler recién creado
  no necesita repintado; invalidar una región lo activa; `invalidate_all`
  se reporta como "repinta todo" sin lista de regiones (`take_dirty_regions`
  devuelve `None`, no una lista con un rectángulo enorme); tomar las
  regiones limpia el estado pendiente (o el mismo repintado se repetiría
  para siempre); 64 invalidaciones pequeñas se fusionan en, como mucho,
  `MAX_REGIONS` regiones, y la unión resultante sigue cubriendo puntos de
  los rectángulos originales (primero y último, verificado); un
  temporizador pedido no está "debido" antes de su hora y sí lo está en
  cuanto llega (y después); de dos temporizadores pedidos, el más
  temprano es el que manda; tomar las regiones también limpia un
  temporizador pendiente.

  **Fuera de alcance, a propósito**: conectar esto a un bucle de eventos
  real (no existe ninguno — `lux-platform` no tiene todavía un bucle de
  render que consulte esto, y no lo tendrá hasta que exista una app
  unificada). No se tocó ningún otro crate ni ningún archivo de
  `lux-render` fuera de `schedule.rs` y `lib.rs` (registro del módulo) —
  sin dependencias nuevas.

  `cargo run -p xtask` en verde de punta a punta (ver el receipt hermano
  del pipeline de quads/texto para el recuento completo de tests del
  workspace en esta misma pasada).

# Receipt: Multicursor y fusión de solapamientos

- Phase: 01
- Crate(s): lux-core
- Goal: N cursores con anchor y head, con normalización (orden por posición
  en el documento) y fusión de solapamientos, de forma que un conjunto de
  cursores nunca contenga dos rangos que cubran el mismo texto ni cursores
  colapsados duplicados en la misma posición.
- Verify: `cargo test -p lux-core -- cursor:: multicursor::`
- Iterations: 1 / 10
- Result: passed
- Opened: 2026-07-27
- Closed: 2026-07-27
- Notes:

  Igual que en el entregable anterior, el comando de gate tal como se
  escribió (`cargo test -p lux-core cursor:: multicursor::`, sin `--`) no es
  ejecutable: `cargo test` solo admite un `TESTNAME` posicional sin `--`; un
  segundo argumento es un error de parseo de `cargo`, no de test. Se usó
  `cargo test -p lux-core -- cursor:: multicursor::` (con `--`), que es la
  forma correcta de pasar dos filtros de nombre al binario de tests.

  **Diseño.** Un "cursor" en este código es exactamente un
  `Selection` (entregable anterior, ya cerrado) — no se creó un tipo nuevo
  que duplicara sus campos `anchor`/`head`; se reutilizó tal cual. Dos
  módulos nuevos, hermanos de `buffer`/`position`/`selection`:

  - `cursor.rs` — el algoritmo puro, sin estado: `normalize` (ordena por
    posición) y `merge_overlapping` (ordena y luego fusiona en una pasada
    lineal cualquier par de cursores que se solapen, coincidan en el mismo
    punto, o donde un cursor colapsado caiga dentro del rango de otro). La
    fusión es transitiva (A solapa B, B solapa C ⇒ los tres se fusionan en
    uno) porque cada nuevo cursor se compara contra el *resultado ya
    fusionado* anterior, no contra los originales. Dos selecciones que solo
    se tocan en un borde (una termina exactamente donde empieza la otra) NO
    se fusionan — mismo criterio que `Selection::overlaps` del entregable
    anterior, reutilizado directamente para el caso no-vacío/no-vacío.
    Decisión explícita sin especificar en el prompt maestro: tras fusionar,
    la selección resultante siempre queda en dirección hacia delante
    (`anchor` antes que `head`), descartando la dirección original de
    cualquiera de los dos cursores fusionados — una elección determinista
    simple, no una limitación real.
  - `multicursor.rs` — `MultiCursor`, la colección con estado: mantiene el
    invariante de "siempre ordenado, nunca solapado" en cada `add`, y
    garantiza al menos un cursor en todo momento (`from_cursors` hace
    `panic` si se le pasa un vector vacío). Rastrea cuál cursor es
    "primario" (el último añadido o movido — necesario para operaciones
    futuras como "seleccionar siguiente ocurrencia") guardando el offset de
    char de su `head` en vez de un índice de vector, precisamente porque una
    fusión puede desplazar o reducir el vector de cursores; `primary()`
    resuelve ese offset contra el conjunto ya fusionado en cada llamada.

  **Fuera de alcance, dejado para el siguiente entregable**: aplicar una
  operación de edición a los N cursores en una sola transacción (texto del
  prompt maestro, entregable 3) — eso depende de que `Buffer` tenga
  operaciones de edición, que todavía no existen (son el entregable 5). No
  se tocó `buffer.rs`, `position.rs` ni `selection.rs` salvo lo ya cerrado en
  entregables previos.

  Sin fallos repetidos ni bloqueos; todo pasó en el primer intento del
  código (17/17 tests nuevos, 59/59 en todo el crate). Nada que amerite
  promoción a `CLAUDE.md`.

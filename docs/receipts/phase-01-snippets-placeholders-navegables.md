# Receipt: Snippets con placeholders navegables

- Phase: 01
- Crate(s): lux-core
- Goal: Snippets con placeholders navegables reales — Tab/Shift+Tab,
  placeholders espejo, valores por defecto — sobre `Buffer`, `Edit`,
  `Position` y `Selection` ya existentes. No la función `strip()` de v0
  (ver ADR 0001): un modelo de tab stops real que se puede recorrer y
  editar.
- Verify: `cargo test -p lux-core snippets::`
- Iterations: 3 / 8
- Result: passed
- Opened: 2026-07-27
- Closed: 2026-07-27
- Notes:

  **Diseño.** Directorio `snippets/`: `parse.rs` (gramática mínima estilo
  TextMate: `$1`, `${1}`, `${1:default}`, `$0`, escapes `\$`/`\\`/`\}` — sin
  placeholders anidados ni transformaciones/variables, deliberadamente
  fuera de alcance) y `mod.rs` (`SnippetSession`: inserta el snippet
  expandido en el buffer y ofrece `next`/`prev`/`current_selection`/
  `update_current`).

  El parser hace dos pasadas: primero trocea la plantilla en segmentos de
  texto/tabstop sin fijar posiciones todavía; luego resuelve, por cada
  índice, un "valor por defecto canónico" (el primer `${N:...}` de ese
  índice en toda la plantilla, aparezca donde aparezca); y solo entonces
  construye el texto final, usando el valor propio de cada ocurrencia si lo
  tiene o el canónico si es una ocurrencia espejo sin llave (`$1` simple).
  Esto importa porque un espejo sin valor propio debe *mostrar* el
  contenido del placeholder principal, no quedar vacío.

  `update_current` (lo que ocurre cuando el usuario escribe sobre el
  placeholder activo) aplica el reemplazo a todas sus ocurrencias espejo a
  la vez y reposiciona el resto de tab stops — incluidos los de OTROS
  índices que puedan estar intercalados en el documento entre los espejos
  del que se está editando. En vez del patrón "de atrás hacia delante" que
  usé en entregables anteriores, aquí proceso las ocurrencias en orden
  ascendente de documento con un acumulador de desplazamiento (`shift`) que
  se va sumando según se aplica cada edición — construye `edits` en el
  orden cronológico correcto de forma directa, sin necesitar revertir nada
  al final.

  **Fallos reales — tres, todos en el primer intento de compilación
  limpia:**

  1. El stop final implícito (`$0` cuando la plantilla no lo pone) se
     insertaba como un `Vec` vacío de rangos en vez de un rango
     `[fin..fin]` — `group_tabstops` hacía `groups.entry(0).or_default()`,
     que crea un vector vacío si el índice 0 nunca apareció, y nada volvía
     a rellenarlo. Corregido con `or_insert_with` que solo actúa cuando el
     índice 0 de verdad no existe, poniendo el rango implícito correcto.
  2. **El bug de diseño real**: las ocurrencias espejo sin llave (`$1`
     simple tras un `${1:x}` previo) no heredaban el valor por defecto del
     placeholder principal — el parser original trataba cada ocurrencia de
     forma aislada, así que `"${1:x} + $1"` producía `"x + "` en vez de
     `"x + x"`. Es la razón del rediseño de dos pasadas descrito arriba.
  3. Con el nuevo helper `update_current`, un test con espejos de un índice
     antes Y después del tab stop que se estaba editando confirmó que el
     reposicionamiento por orden ascendente + acumulador de desplazamiento
     funciona correctamente incluso cuando los grupos se entrelazan en el
     documento — este test pasó a la primera una vez corregido el bug del
     parser (no reveló un bug adicional en `update_current` mismo).

  Se prestó atención deliberada, aprendiendo del entregable anterior de
  buscar/reemplazar, a NO añadir un `.reverse()` de más al final de
  `update_current` — se verificó explícitamente con un test
  (`update_current_is_undoable`) que invierte los `Edit` devueltos a mano y
  comprueba que el buffer vuelve exactamente al estado previo a la
  inserción del snippet.

  Nada que amerite promoción a `CLAUDE.md` más allá de lo ya anotado en el
  receipt de buscar/reemplazar sobre probar siempre el camino de undo.

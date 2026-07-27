# Receipt: Buscar/reemplazar en buffer

- Phase: 01
- Crate(s): lux-core
- Goal: Buscar y reemplazar dentro del buffer — literal, insensible a
  mayúsculas, palabra completa, regex, con reemplazo por capturas — sobre
  `Buffer`, `Position`, `Selection`, `Edit` y `Transaction` ya existentes.
- Verify: `cargo test -p lux-core find_replace::`
- Iterations: 2 / 8
- Result: passed
- Opened: 2026-07-27
- Closed: 2026-07-27
- Notes:

  **Diseño.** Un solo fichero, `find_replace.rs`. `Query` (patrón +
  `case_sensitive`/`whole_word`/`regex` encadenables) compila SIEMPRE a un
  único `regex::Regex`, para los cuatro modos a la vez: el modo literal usa
  `regex::escape()` sobre el patrón antes de compilar, palabra completa
  envuelve con `\b...\b`, insensible a mayúsculas antepone `(?i)`. Evita
  mantener un motor de búsqueda literal aparte del motor regex — un único
  camino de código para los cuatro modos combinables. `find_all`/`find_next`
  reutilizan `Position::from_byte` (del entregable de posiciones) para
  convertir los offsets de byte que devuelve `regex` a `Position`
  completos. El reemplazo por capturas usa `Captures::expand` de la propia
  crate `regex` (sintaxis `$1`, `${nombre}`) en modo regex; en modo literal
  el texto de reemplazo se inserta tal cual, sin interpretar `$`.
  `replace_one`/`replace_all` devuelven una `Transaction` (del entregable de
  undo/redo) lista para que quien llame la aplique vía `History` — ninguna
  de las dos toca `History` directamente.

  **Dependencia añadida**: `regex` 1.13.1 — no estaba en la tabla de
  `docs/DEPENDENCIES.md` (el prompt maestro no la menciona explícitamente
  para este entregable, a diferencia de `ignore`/`grep-searcher`/
  `grep-regex`, que son para búsqueda a nivel de proyecto tipo ripgrep, un
  entregable distinto y posterior). No se tocó `docs/DEPENDENCIES.md` por
  estar fuera de alcance; se documenta aquí en su lugar.

  **Fallos reales — dos, ambos en la primera compilación limpia:**

  1. Dos errores de aritmética mía en los tests (rangos de coincidencias
     mal calculados a mano en `"cat cats category"` y en la búsqueda de
     palabra completa) — corregidos verificando con un script Python de una
     línea, sin tocar el código bajo prueba.
  2. **Un bug real en `replace_all`**: el bucle que aplica las
     sustituciones de atrás hacia delante (para que los offsets de las
     coincidencias anteriores sigan siendo válidos mientras se procesan las
     posteriores) ya construye `edits` en el orden cronológico correcto
     mediante `push()` según se van aplicando de verdad al buffer. Añadí un
     `edits.reverse()` extra al final — copiado por costumbre de un patrón
     usado en `edit_ops` para otro propósito (dejar la lista en orden de
     documento de cara a quien la lea) — que revolvía ese orden y dejaba
     `Transaction.edits` invertido. El síntoma: `replace_all` sustituía bien
     el texto, pero `transaction.undo()` no restauraba el original (el test
     `replace_all_is_undoable_as_one_transaction` lo detectó al primer
     intento). Corregido eliminando ese `.reverse()` final — la lista ya
     estaba bien construida sin él.

  **Hallazgo importante fuera de alcance, señalado pero no corregido aquí**:
  al depurar el bug anterior, confirmé que **la misma clase de fallo existe
  en `edit_ops::apply_to_each`** (entregable "Operaciones de edición",
  cerrado dos turnos atrás): ese `all_edits.reverse()` final tiene
  exactamente el mismo problema — la lista ya estaba en orden cronológico
  correcto antes de esa línea, y el reverso la invierte. `apply_to_each` es
  precisamente la función que aplica una edición a N cursores en una sola
  transacción, así que este bug significa que **deshacer una edición
  multicursor construida con `apply_to_each` probablemente no restaura el
  estado original**. No se detectó en su momento porque ninguno de los
  tests de ese entregable llama a `transaction.undo()` sobre el resultado
  de `apply_to_each` — solo comprueban el texto final y las posiciones de
  cursor resultantes, que no dependen del orden de `edits`. No se corrigió
  en este turno por estar fuera del alcance explícito de este entregable
  ("buscar/reemplazar", no "operaciones de edición"); queda documentado
  aquí y se lo señalo directamente al usuario para que decida si abre un
  `/goal` de corrección específico.

  **Para promover a memoria/CLAUDE.md**: cuando una función construye una
  lista de `Edit` recorriendo algo "de atrás hacia delante" y usa `push()`
  en el orden real de aplicación, esa lista YA queda en el orden
  cronológico correcto — no hace falta (y es un error) revertirla al final
  a menos que el propósito explícito sea presentarla en orden de documento
  para un consumidor externo que no vaya a deshacerla. Cualquier función
  que construya una `Transaction` debería tener al menos un test que
  ejercite `transaction.undo()` sobre su resultado, no solo el estado final
  del buffer — así es como se detectó este bug y así se habría detectado el
  de `apply_to_each` si hubiera existido esa cobertura entonces.

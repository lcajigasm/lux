# Receipt: Buffer (ropey, guardado atómico, detección EOL/indent/encoding)

- Phase: 01
- Crate(s): lux-core
- Goal: `Buffer` sobre `ropey` 1.6.1 con carga, guardado atómico con backup,
  detección de finales de línea (LF/CRLF/CR), detección de estilo y anchura
  de indentación, y detección/conversión de codificación (UTF-8, UTF-16 con
  BOM, Latin-1) con detección de ficheros binarios.
- Verify: `cargo test -p lux-core buffer::`
- Iterations: 3 / 10
- Result: passed
- Opened: 2026-07-27
- Closed: 2026-07-27
- Notes:

  `lux-core` estaba vacío (solo `lib.rs` en blanco). Se creó el módulo
  `buffer/` con tres submódulos:

  - `eol.rs` — `Eol` (Lf/Crlf/Cr), detección por conteo y mayoría.
  - `indent.rs` — `Indent { style, width }`, detección de tabs vs. espacios
    y anchura por voto sobre el delta de indentación entre líneas
    consecutivas (heurística estándar de "guess indentation").
  - `encoding.rs` — `Encoding` (Utf8, Utf8Bom, Utf16Le, Utf16Be, Latin1),
    `decode`/`encode` simétricos, y `looks_binary` (heurística de NUL byte
    en los primeros 8000 bytes, la misma que usa git/ripgrep).
  - `mod.rs` — `Buffer` que ata todo: `from_bytes`, `open`, `save`
    (atómico: escribe a un fichero temporal en el mismo directorio, fsync,
    backup del fichero previo a `<nombre>.bak`, y `rename` final).

  Dependencias añadidas a `lux-core`: `ropey = "1.6.1"` (fija según ADR
  0007). No se añadió ningún crate de encoding — `std::char::decode_utf16`/
  `encode_utf16` cubren UTF-16, y Latin-1 es un mapeo directo byte↔codepoint
  que no necesita librería.

  **Fallo repetido y su causa raíz**: la primera versión de `decode()`
  comprobaba "¿es binario?" (byte NUL en los primeros 8000 bytes) antes de
  comprobar el BOM de UTF-16. Pero en UTF-16, todo carácter ASCII tiene el
  byte alto en `0x00`, así que cualquier texto UTF-16 con contenido ASCII
  disparaba el detector de binario como falso positivo — los 4 tests de
  UTF-16 fallaban con `Err(Binary)`. La causa raíz no era el heurístico de
  binario en sí, sino el orden: se corrigió moviendo la comprobación de los
  BOMs de UTF-16 antes del chequeo de binario, en vez de intentar "arreglar"
  el heurístico de NUL (que sigue siendo correcto para todo lo que no es
  UTF-16). Un solo cambio de orden resolvió las 4 fallas a la vez.

  Otros dos ajustes menores durante el loop: `Buffer` necesitaba
  `#[derive(Debug)]` porque los tests usan `unwrap_err()` sobre
  `Result<Buffer, _>`; y clippy pidió `slice::contains(&0)` en vez de
  `iter().any(|&b| b == 0)` en `looks_binary`.

  Nada de esto merece promoción a `CLAUDE.md`: son detalles de
  implementación de este entregable, no correcciones de proceso repetibles
  en otros contextos.

# Receipt: Cierre — fuzzing 30 min + cobertura ≥85%

- Phase: 01
- Crate(s): lux-core, lux-core (fuzz/, nuevo subcrate standalone)
- Goal: Dos criterios de aceptación de la Fase 1: cobertura de tests de
  `lux-core` ≥ 85%, y fuzzing (`cargo-fuzz`) sobre carga de buffer y
  aplicación de ediciones, 30 minutos sin crash ni panic.
- Verify: `cargo fuzz run buffer_fuzz -- -max_total_time=1800`
- Iterations: 3 / 8 (bloqueo de entorno + 2 de ajuste, ver notas)
- Result: passed
- Opened: 2026-07-27
- Closed: 2026-07-27
- Notes:

  **Bloqueo de entorno, resuelto con autorización explícita del usuario.**
  `cargo fuzz` exige un toolchain nightly; el entorno solo tenía Rust
  estable vía Homebrew, sin `rustup`. Antes de tocar nada fuera del repo
  pregunté al usuario cómo proceder (instalar, parar, o que lo instalara
  él); eligió que instalara. Se instaló `rustup` con `--no-modify-path`
  (para no alterar el `PATH` global ni interferir con el toolchain estable
  ya usado por todo el proyecto), se añadió el toolchain `nightly` aparte,
  y se instaló `cargo-fuzz` 0.13.2 y `cargo-llvm-cov` 0.8.7. Todos los
  comandos de fuzzing/cobertura invocan sus binarios con ruta explícita
  (`$HOME/.cargo/bin`) o `+nightly`; el resto del repo sigue resolviendo
  `cargo`/`rustc` 1.97.1 estable vía Homebrew exactamente como antes,
  según pineado en `rust-toolchain.toml`.

  **Cobertura**: `cargo llvm-cov -p lux-core` da **98.50% de regiones,
  98.79% de funciones, 98.30% de líneas** sobre los 193 tests de la lib más
  el proptest de integración — muy por encima del 85% exigido. Los
  ficheros con menor cobertura (`position.rs` 90.65%, `edit_ops/word.rs`
  96.48%, `selection_ops/occurrence.rs` 94.86%) corresponden a ramas de
  guarda defensivas (límites de documento vacío, casos `unreachable!` de
  `GraphemeCursor`) que ya tienen su propio test explícito, no huecos de
  cobertura reales sin probar.

  **Fuzzing**: `crates/lux-core/fuzz/` es un subcrate `cargo-fuzz`
  independiente del workspace principal (`[workspace]` vacío en su
  `Cargo.toml`, a propósito, para no arrastrar el workspace raíz —pineado
  en estable— a depender de nightly). El target `buffer_fuzz` usa
  `arbitrary` para generar bytes iniciales de fichero más una secuencia
  estructurada de operaciones `Insert`/`Delete`, con los offsets recortados
  al rango válido del buffer en el momento de aplicarse (mismo patrón de
  `tests/undo_proptest.rs`), para que cada caso generado sea una edición
  legal y cualquier pánico encontrado sea un bug real, no un artefacto del
  arnés. Corrida completa: **53 713 879 ejecuciones en 1801 segundos, sin
  crash, sin pánico, sin artefactos de fallo generados**.

  **Fallo real durante el setup (no del núcleo, del andamiaje de
  fuzzing)**: `cargo fuzz init` generó un `fuzz/Cargo.toml` con
  `[dependencies.lux]` en vez de `[dependencies.lux-core]` — al parecer
  confundido por el nombre del directorio raíz del repo (`lux`) en vez del
  nombre real del paquete bajo prueba (`lux-core`). Corregido a mano, y se
  añadió `[workspace]` vacío (ausente en la plantilla generada) para
  aislar el subcrate del workspace raíz.

  **Alcance de este goal**: cubre únicamente estos dos criterios de
  aceptación de los cinco que tiene la Fase 1 en `LUX_PROMPT_MAESTRO.md`.
  Los otros tres — benchmarks de rendimiento, `cargo tree -p lux-core` sin
  dependencias gráficas, y compilación para `aarch64-apple-ios` /
  `aarch64-linux-android` — quedan fuera del alcance de este goal
  concreto; ver el informe de `/close-phase 1` para el estado completo de
  cierre de fase, que además encontró que el entregable 6 ("Movimiento")
  nunca se abordó.

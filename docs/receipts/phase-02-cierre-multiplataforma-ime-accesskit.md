# Receipt: cierre multiplataforma — IME de escritorio y `accesskit` conectado (entregables 5 y 6)

- Phase: 02
- Crate(s): lux-platform, lux-ui (sin cambios de código — solo verificación)
- Goal: Determinar, de forma concreta y no por suposición, si queda algo
  alcanzable en esta máquina para terminar de verificar los entregables 5
  ("IME de escritorio... en las tres plataformas") y 6 (`accesskit`
  conectado, "VoiceOver, NVDA, Orca") más allá de lo que sus propios
  receipts ya cerraron en macOS.
- Verify: comprobación directa de la disponibilidad de toolchains cruzados
  (`rustup`, `rustc --print sysroot`) — no hay gate de test nuevo, esto no
  añade código
- Iterations: 1
- Result: passed (como "bloqueo confirmado y documentado", no como "hecho")
- Opened: 2026-07-30
- Closed: 2026-07-30
- Notes:

  **Por qué este receipt y no más código.** Los entregables 5 y 6 ya
  tenían su trabajo de funcionalidad completo y verificado en macOS con
  entradas reales (`docs/receipts/phase-02-ime-escritorio-preedit-area-
  cursor.md`, `docs/receipts/phase-02-accesskit-conectado.md`) — ambos con
  veredicto explícito del propietario del proyecto. Lo único que quedaba
  abierto en los dos era la verificación en Windows/Linux y con NVDA/Orca.
  La regla de oro nº10 pide parar y reportar cuando un objetivo es
  inalcanzable, en vez de dejarlo ambiguo — así que en vez de reintentar
  algo que ya se sabía bloqueado, se comprobó de forma directa y concreta
  si de verdad no hay nada más que hacer desde esta sesión.

  **Comprobación realizada**:
  ```
  $ rustup target list --installed
  zsh: command not found: rustup
  $ rustc --version
  rustc 1.97.1 (8bab26f4f 2026-07-14) (Homebrew)
  $ which rustc
  /opt/homebrew/bin/rustc
  $ rustc --print sysroot
  /opt/homebrew/Cellar/rust/1.97.1
  $ find "$(rustc --print sysroot)/lib/rustlib" -maxdepth 1 -type d
  /opt/homebrew/Cellar/rust/1.97.1/lib/rustlib/aarch64-apple-darwin
  /opt/homebrew/Cellar/rust/1.97.1/lib/rustlib/etc
  /opt/homebrew/Cellar/rust/1.97.1/lib/rustlib/src
  ```

  **Conclusión, confirmada de forma directa, no asumida**: este Rust viene
  de Homebrew, no de `rustup`, y Homebrew solo empaqueta la biblioteca
  estándar del target anfitrión (`aarch64-apple-darwin`). Sin `rustup` no
  hay forma de instalar el sysroot de ningún otro target — ni siquiera
  para un `cargo check` (que no necesita enlazador, solo necesita la
  `std`/`core` de ese target instalada, y no lo está ni se puede instalar
  sin `rustup`). Esto confirma y refuerza — con un diagnóstico directo, no
  por repetir lo ya asumido — la misma conclusión que ya dejó escrita el
  receipt del entregable 1 de `lux-platform` ("unlike Phase 1's pure-Rust
  `lux-core`, `lux-platform`'s desktop backend pulls in real per-OS system
  dependencies... so a cross-compile check isn't meaningful here"). No es
  un hueco dejado por descuido: es un límite real y verificado de esta
  máquina.

  **Qué significa esto para los entregables 5 y 6**: ambos quedan
  completos en todo lo que es código y verificación real posibles desde
  aquí. Lo que sigue pendiente — NVDA y Orca para `accesskit`, cualquier
  IME real de Windows/Linux — necesita una máquina Windows o Linux de
  verdad; no hay ninguna acción de software, configuración o reintento
  que lo destrabe desde esta sesión. Queda documentado como bloqueo de
  entorno cerrado, no como trabajo abierto.

  **Fuera de alcance, a propósito**: no se tocó ningún archivo de código
  de `lux-platform` ni de `lux-ui` en este receipt — es puramente una
  verificación de entorno, sin cambios que probar con `cargo test`.

  `cargo run -p xtask` no aplica aquí (sin cambios de código); el estado
  del workspace sigue siendo el mismo verde de punta a punta que dejaron
  los receipts hermanos de esta misma pasada
  (`phase-02-luxrender-pipeline-quads-texto-cache-textrenderingmode.md`,
  `phase-02-loop-render-presupuesto.md`).

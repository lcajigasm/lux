# Receipt: Packaging helpers (Phase 11)

- Phase: 11
- Crate(s): lux-pack, lux
- Goal: Local crash files (no upload); Ed25519 verify of update payloads;
  unsigned dist layout script; winget stub; CONTRIBUTING/CoC/issue template.
- Verify: `cargo test -p lux-pack`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-09-01
- Closed: 2026-09-01
- Notes:

  Notarization, signed MSI/dmg, and a live auto-update server are **not**
  done — they need vendor credentials. STATUS rows stay `not-started` for
  signed installers. Crash + signature verify are real.

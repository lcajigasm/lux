# Non-goals

Lux is not an IDE and does not compete with VS Code on breadth. The following
are out of scope, without exception, for the entire cycle through v1.0:

- Debugging / DAP / breakpoints / call stacks.
- Real-time collaboration, multi-user presence, follow mode.
- A hosted, signed, or remotely discoverable extension marketplace.
- Cloud settings sync, user accounts, login.
- Telemetry (not even opt-in; the subsystem does not exist in v1.0).
- AI assistants, model-based autocomplete, integrated chat.
- Notebooks, data preview, database clients.
- Lux-specific refactorings beyond what the LSP exposes.
- Web / WASM-in-browser support.

These may be reopened after v1.0, each with its own phase cycle. Not before.

This list is binding per golden rule #7 in `CLAUDE.md`. A good idea that falls
here goes in `docs/IDEAS.md`, not into the codebase.

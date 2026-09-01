# lux-plugin API

Host: `wasmtime` 47 (Component Model), ADR 0016.

## Load and run

`PluginHost::new(grants).run_component(wasm)` instantiates a component and
calls exported `run: func() -> u32` (returned as a decimal string), with
fuel and a memory cap.

## Permissions

`PermissionSet` JSON at `.lux/plugin-grants/<id>.json`. Capabilities:
`file-read`, `file-write`, `network`, `process`. Missing grant →
`HostError::Denied`, never a successful I/O.

## Install

`install_from_dir(src, dest)` copies a folder that contains
`lux-plugin.json` and `trust.json` with `"trusted": true`. No trust file
or `"trusted": false` → `InstallError::Untrusted`.

## Extension points

Manifest fields: `commands`, `grammars`, `themes`, `formatters`,
`lsp_servers`, `decorations`. Data files (themes) are copied as-is; WASM
guests are optional.

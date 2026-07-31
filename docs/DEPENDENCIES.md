# Fixed dependencies

Original research: 2026-07-25 (see `LUX_PROMPT_MAESTRO.md` section 0.6).
Re-verified against crates.io: **2026-07-26**, per the Phase 0 mandate to
recheck before building on top of any of these. None of the checked crates
had a material change (no stable `winit` 0.31, no glyph atlas in `vello`, no
RUSTSEC advisory newly introduced and unpatched, `gpui` unchanged). Two minor
corrections to the original table are noted below the main table. Re-verify
again before Phase 2 (the render stack) and before any major-version bump.

None of these are wired into a crate yet — Phase 0 crates are empty stubs by
design (see `docs/adr/0001-lessons-from-v0.md` and the master prompt's
explicit "no UI in this phase" instruction). This table is what each phase
pulls in when it needs it.

| Area | Crate(s) | Version (verified 2026-07-26) | Note |
|---|---|---|---|
| GPU | `wgpu` | 30.0.0 | Breaking major every ~12 weeks by policy. Confirmed live by temporarily adding it to `lux-core` during the core-isolation test (see receipt) — resolves clean at 30.0.0. Wired for real into `lux-render` in Phase 2 (context/surface/resize); pulls in one new license (`Zlib`, via `foldhash`/`slotmap` under `wgpu-hal`) — permissive, added to `deny.toml`. |
| Window/display handles (in `lux-render`) | `raw-window-handle` | 0.6.2 | Same crate `lux-platform` uses; `lux-render` takes it directly too, deliberately not depending on `lux-platform` (see `docs/ARCHITECTURE.md` — a future mobile backend can hand `lux-render` a surface without pulling in windowing/IME/clipboard code). |
| DPI geometry (in `lux-render`) | `dpi` | 0.1.2 | Same reasoning as `lux-platform`'s use of it: shared, `wgpu`/`winit`-independent size vocabulary. |
| Async-to-sync bridge for tests | `pollster` | 1.0.1 | `wgpu`'s adapter/device requests are `async`; this blocks on them from plain `#[test]` functions. Dev-dependency only. |
| Desktop windowing | `winit` | 0.30.13 | 0.31 still beta (0.31.0-beta.2, 2025-11-16), no stable release since. Design the `lux-platform` trait against 0.31's shape anyway. Built with `default-features = false` plus everything default *except* `wayland-csd-adwaita`, replaced by `wayland-csd-adwaita-crossfont`: the `ab_glyph`-based default pulls in `ttf-parser`, unmaintained per RUSTSEC-2026-0192 (found wiring up `lux-platform`, Phase 2). `crossfont` gives the same Wayland CSD support without it. |
| Window/display handles | `raw-window-handle` | 0.6.2 | Arrives via `winit`'s `rwh_06` feature; `lux-platform`'s `PlatformWindow` trait requires it directly too, as the shared vocabulary a future `wgpu` surface target needs regardless of backend. |
| DPI geometry | `dpi` | 0.1.2 | The standalone crate `winit` itself re-exports as `winit::dpi` (`pub use dpi;` in `winit`'s `lib.rs`) — depending on it directly keeps `lux-platform`'s size/position types identical to `winit`'s without requiring `winit` itself on non-desktop backends later. |
| Clipboard | `arboard` | 3.6.1 | Not cited in the master prompt (an omission — `winit` has no clipboard of its own). `default-features = false`: only text clipboard is used, not the `image`/`image-data` features. |
| Native file dialogs | `rfd` | 0.17.2 | Not cited in the master prompt either. `default-features = false`: the synchronous `FileDialog` API needs none of the `gtk3`/`xdg-portal`/`pollster` opt-in features. |
| Text layout | `parley` + `fontique` | 0.11.0 | Wired for real into `lux-render` (Phase 2, "integración parley+fontique layout/ligaduras/clusters"); both resolve clean, still latest as of 2026-07-30. Built with `default-features = false, features = ["std"]` on both — **not** `system` (fontique's platform font-discovery feature): enabling it pulls `yeslogic-fontconfig-sys` 6.x, which conflicts at the Cargo `links = "fontconfig"` level with the 5.x version `crossfont` pins (via `winit`'s `wayland-csd-adwaita-crossfont`, already required by `lux-platform` — see this crate's own row above). Both crates share one workspace-wide resolved dependency graph, so this is a hard conflict, not a per-crate preference; the fix is registering fonts directly from bytes (`Collection::register_fonts`) instead of system enumeration, which is real future work, not a placeholder — tracked as a documented gap in `docs/STATUS.md`. |
| Shaping | `harfrust` | 0.10.0 | **Correction to the 2026-07-26 re-verification**: that pass recorded 0.12.0 (harfrust's own latest standalone release, confirmed still current) but didn't check what `parley` 0.11.0 actually pins — its `Cargo.toml` requires `harfrust = "0.10.0"` (implicit caret, so `>=0.10.0, <0.11.0`), not 0.12.0. Confirmed by resolving the real dependency graph while wiring `parley` into `lux-render`: `harfrust` 0.10.0 is what's actually in `Cargo.lock`. Also produces a duplicate-version warning (non-fatal, `bans.multiple-versions = "warn"`) against `read-fonts`/`skrifa`, which `swash` pins to newer versions (0.41.0/0.44.0) than `parley`/`fontique` do (0.40.2/0.43.2) — both version pairs coexist in the tree; not a build error, just two copies of each. |
| Rasterizing | `swash` | 0.2.10 | Only crate offering `Format::subpixel_bgra()`. Wired for real into `lux-render`'s glyph atlas (Phase 2, "atlas de glifos etagere+swash 4×1 subpíxel"); resolves clean at 0.2.10, pulls in `zeno` 0.3.3 (rasterization), `skrifa`/`read-fonts`/`font-types` (Fontations, font parsing) and `yazi` (embedded-bitmap decoding), all permissive-licensed, no new `deny.toml` entry needed. |
| Glyph atlas | `etagere` | 0.3.0 | Not version-pinned in the master prompt; 0.3.0 is current. Reference design: `glifo` 0.1.1 — see note below, it turned out to be less independent than assumed. Wired for real alongside `swash` in the same receipt; `BucketedAtlasAllocator` chosen over `AtlasAllocator` per its own docs' recommendation for "a very large amount of small items" — exactly the glyph case. |
| GPU-buffer byte casting (in `lux-render`) | `bytemuck` | 1.25 | Not cited in the master prompt. Already a transitive dependency (via `font-types`/`read-fonts`/`skrifa`, themselves already in the tree); added directly to safely cast `QuadInstance`/`GlyphInstance` vertex-buffer structs to bytes instead of hand-rolled `unsafe` transmutes, for the quad/text pipeline (Phase 2 deliverable 2's remaining slice). Zero new compile cost — it was already being built. |
| Accessibility | `accesskit` | 0.24.1 | |
| — | `accesskit_windows` | 0.34.0 | |
| — | `accesskit_macos` | 0.26.3 | |
| — | `accesskit_unix` | 0.22.1 | |
| — | `accesskit_android` | 0.7.5 | |
| — | `accesskit_ios` | 0.1.2 | Experimental, ~2 months old as of Phase 0. Budget debugging time in Phase 12. |
| Text buffer | `ropey` | 1.6.1 | `2.0.0-beta.1` still stalled since 2025-08-02. Stay on 1.6.1. |
| Syntax | `tree-sitter` | 0.26.11 | No 1.0. Wired for real in Phase 3 (`lux-syntax`, incremental highlighting for the fixed 18-grammar set — ADR 0008 has the exact list; note it reads "17" in prose but TypeScript/TSX are two separate grammars, so the compiled-in set is 18). Grammars compiled into the binary (issue #3095 workaround), not depended on as Cargo crates — see ADR 0008 (corrects this row's earlier "ADR 0009" typo). |
| Syntax (grammar C sources) | `cc` | 1.4.0 | Build-dependency only. Compiles each vendored grammar's generated `parser.c`/`scanner.c` in `lux-syntax/build.rs`. |
| Syntax (grammar entry points) | `tree-sitter-language` | 0.1.7 | Tiny `no_std` crate providing `LanguageFn`, the safe wrapper `tree-sitter` itself uses around a grammar's raw `extern "C" fn tree_sitter_<lang>() -> *const ()`. Independently versioned from `tree-sitter` core (0.1.x, not 0.26.x) — that's expected, it's a stable ABI-only shim, not a grammar. |
| Search | `ignore` | 0.4.31 | ripgrep family. |
| Search | `grep-searcher` | 0.1.17 | ripgrep family. |
| Search | `grep-regex` | 0.1.14 | **Correction**: the master prompt cited "~0.4.25", which is not a version that exists for this crate (likely confused with the `grep` facade crate or `globset`). 0.1.14 is this crate's actual latest. Does not change the choice. |
| Fuzzy matching | `nucleo` | 0.5.0 | Stalled since April 2024, confirmed still stalled. Works, used by Helix. Write characterization tests so a fork/replacement is cheap if it ever blocks us. |
| FS watchers | `notify` | 8.2.0 | 9.0.0-rc.4 still an RC. Use 8.2.0 + debouncer. |
| Git | `gix` | 0.86.0 | Bumped from the cited 0.84.0 (minor, non-breaking). In-process only, no shell-out. |
| LSP | `async-lsp` | 0.2.4 | Option A. |
| LSP | `lsp-server` | 0.10.0 | Option B. |
| LSP | `tower-lsp-server` | 0.23.0 | Living fork of the abandoned `tower-lsp`. |
| LSP types | `lsp-types` | 0.97.0 | Confirmed stalled since 2024-06-04 — avoid depending on it directly if `tower-lsp-server`'s `ls-types` covers the need. |
| — (avoid) | `tower-lsp` | 0.20.0 | Confirmed abandoned since 2023-08-11. Do not use. |
| Terminal | `portable-pty` | 0.9.0 | Confirmed stalled since 2025-02-11. Works, has adoption. Same fork-risk note as `nucleo`. |
| Terminal | `alacritty_terminal` | 0.26.0 | |
| Plugins | `wasmtime` | 47.0.2 | Newer than the cited 45.0.1; no unpatched RUSTSEC advisory either way. Pin the exact version used in Phase 9's ADR. |
| Config | `serde` + JSONC | — | No fixed version yet; picked when Phase 5 lands. |

## Discarded, re-checked for a change of mind

| Crate | Verdict at 2026-07-25 | Re-verified 2026-07-26 |
|---|---|---|
| `gpui` | Discarded: crates.io release (0.2.2) broken and unmaintained as a library, no mobile support. | Unchanged: still 0.2.2, no release since 2025-10-22. Discard stands. |
| `vello` | Discarded: no glyph atlas (issue #204), alpha status, no subpixel AA. | Issue #204 still open. Note: the `glifo` crate — cited in the master prompt as an independent reference implementation of the same 4×1 subpixel-bucket design — actually moved into the `vello` repo itself and exists specifically to iterate on `vello`'s own glyph-atlas gap. It is not the independently-converged validation the master prompt assumed; treat it only as a design reference for our own atlas, not as evidence that `vello`'s gap is closing on its own. Discard stands. |
| `glyphon` | Fallback path if Phase 2's custom atlas fails its budget gate. | 0.12.0, aligned to `wgpu` 30. Still a viable fallback; decision on whether to use it is explicitly the project owner's, per the Phase 2 decision gate. |

## How to re-verify

    cargo tree -p <crate>              # after adding, confirms resolved version
    open https://crates.io/crates/<name>  # confirms latest published + RUSTSEC status

Re-run this check at the start of Phase 2 (render stack) and Phase 7 (LSP)
at minimum — those are the two areas with the fastest-moving dependencies
(see Annex B, "known risks", in the master prompt).

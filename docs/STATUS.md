# Status

Source of truth for what actually works, per platform (golden rule #9).
Updated in the same PR that changes behavior. The README promises nothing
this file does not confirm.

Legend: `not-started` · `partial` (behind an `experimental.*` flag, listed
under Experimental below) · `implemented` · `excluded` (permanently out of
scope for that platform by design, not a gap to fill — see the linked ADR).

Phase 0 shipped no UI by design (the master prompt is explicit: "if there's a
window at the end of Phase 0, you moved too fast"). Phase 1 shipped a
complete, headless `lux-core` — see below.

## Core editing (Phase 1)

`lux-core` has no platform-specific code (that's the whole point of golden
rule #2); the "implemented" mark below reflects where it's actually been
*run* — the full test suite (235 tests + a proptest + 30 minutes of
`cargo-fuzz`), all on macOS (aarch64-apple-darwin). It's confirmed to
**cross-compile cleanly for `aarch64-apple-ios` and `aarch64-linux-android`**
(`cargo build -p lux-core --target ...`, no linker needed — it's an `rlib`,
no final binary) — Phase 1's specific portability check — but that's a build
check, not a behavioral one, so those columns stay `not-started` until the
suite actually runs there. Windows/Linux are untested for the same reason
(no non-macOS dev environment available yet); the code itself uses only
`std::fs`/`std::path`, nothing OS-specific.

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Buffer load/save (atomic + backup) | implemented | not-started | not-started | not-started | not-started |
| EOL / indent / encoding detection | implemented | not-started | not-started | not-started | not-started |
| Positions/selections (byte/char/column, grapheme-aware) | implemented | not-started | not-started | not-started | not-started |
| Multicursor + multiselection | implemented | not-started | not-started | not-started | not-started |
| Transactional undo/redo | implemented | not-started | not-started | not-started | not-started |
| Edit operations (indent, comment, duplicate, sort, case) | implemented | not-started | not-started | not-started | not-started |
| Movement (word/line/paragraph/bracket-match) | implemented | not-started | not-started | not-started | not-started |
| Sublime-style selections (expand, next occurrence, box) | implemented | not-started | not-started | not-started | not-started |
| Find/replace in buffer | implemented | not-started | not-started | not-started | not-started |
| Snippets with navigable placeholders | implemented | not-started | not-started | not-started | not-started |
| `lux-core-cli` headless test harness | implemented | not-started | not-started | not-started | not-started |

**Closed gap** (2026-08-12, see `docs/PERFORMANCE.md`'s measurement
history): the "open a 100 MB file" performance budget's RSS half (< 3×
file size) measured 3.16× at Phase 1 close, over budget — three
simultaneous copies of the file's bytes alive at once during
`Buffer::open` (the raw `Vec<u8>`, a decoded `String`, and `ropey`'s own
chunk storage). Left open at the time rather than fixed under close-phase
time pressure, revisited in this pass: `Buffer::open` now goes through a
new `encoding::decode_owned`, which reuses the `Vec<u8>` from `fs::read`
as the decoded `String` directly (`String::from_utf8`, zero-copy) on the
common no-BOM-valid-UTF-8 path, cutting the three simultaneous copies to
two. `Buffer::from_bytes(&[u8])` keeps its original borrowing signature
unchanged — the 250-call-site blast radius the original close-phase
report flagged as the reason not to widen it — since only `open`, the
path the budget actually measures, needed the fix. Re-measured the same
way the original number was produced (`/usr/bin/time -l` around
`lux-core-cli` opening a real 100 MB file): **2.16×** (215.6 MB RSS /
100.0 MB file), inside budget.

## Rendering & desktop shell (Phase 2, `v0.1.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Window/input/IME platform trait | implemented | not-started | not-started | not-started | not-started |
| GPU text rendering (glyph atlas, subpixel AA) | implemented | not-started | not-started | not-started | not-started |
| Desktop IME (preedit, cursor area) | implemented | not-started | not-started | n/a | n/a |
| Accessibility tree (`accesskit`) | implemented | not-started | not-started | not-started | not-started |
| Lux Dark theme | implemented | not-started | not-started | not-started | not-started |
| Virtualized editor view | implemented | not-started | not-started | not-started | not-started |

**Scope note (`lux-platform` trait + `winit` backend — this phase's first
deliverable, tracked in `docs/receipts/phase-02-luxplatform-trait-impl-winit.md`)**:
the trait covers window creation, keyboard/mouse/touch input, IME event
plumbing (`Enabled`/`Preedit`/`Commit`/`Disabled`, cursor-area reporting),
clipboard, native file dialogs, DPI/scale-factor, lifecycle
(`resumed`/`suspended` split from `surfaces_available`/`surfaces_lost` per
ADR 0004), and safe-area insets (always zero on desktop — an accurate
report, not a placeholder). Verified on macOS (aarch64-apple-darwin):
`cargo test -p lux-platform` (event-conversion unit tests plus a real
clipboard round-trip against the system pasteboard) and a manual run that
opened a live window and logged genuine resize/focus/occlusion/scale-factor/
mouse events. **Not yet exercised**: live IME composition (Japanese input,
Spanish dead-key accents — Phase 2 deliverable 5, blocked on having
something to render preedit text into, i.e. `lux-render`/`lux-ui`), touch
input (no touch hardware on the dev machine), and file dialogs (a native
modal can't be driven from an automated test, and wasn't manually clicked
through this pass either). Windows and Linux stay `not-started`: this
machine has no `rustup`-managed toolchain, so there's no target sysroot to
even `cargo check` against — unlike Phase 1's pure-Rust `lux-core`,
`lux-platform`'s desktop backend pulls in real per-OS system dependencies
(`windows-sys`, `x11rb`, Wayland), so a cross-compile check isn't meaningful
here the way it was there.

**Scope note (`lux-render`'s `wgpu` context/surface/resize — this phase's
second deliverable, tracked in
`docs/receipts/phase-02-luxrender-contexto-wgpu-superficie-resize.md`)**:
no capability row above moves yet — this is infrastructure underneath "GPU
text rendering", not the row itself, since there's still no glyph atlas, no
quad/text pipeline, and nothing a human can see rendered. What exists and is
verified: `GpuContext` (real headless instance/adapter/device/queue,
`cargo test -p lux-render context::`) and `Surface` (real `wgpu::Surface`
built from a live window's raw handles via `lux-platform`, sRGB-preferring
format selection, resize that's a no-op when the size didn't change,
`cargo test -p lux-render surface::`). Verified manually on macOS: a real
window + real surface configured at its actual size/scale/format
(`Bgra8UnormSrgb` at 1600×1200, matching a Retina display), resized live as
the window resized, and `get_current_texture()` called against the live
surface each redraw attempt (correctly reporting `Occluded` for a
non-frontmost window rather than crashing) — a full clear-color present
wasn't confirmed on screen (the manual test window couldn't be reliably
brought to the foreground from this scripted session), but nothing in that
gap is unverified logic: it's `lux-render`'s render-pass code exercising
the real surface right up to the point a human would need to look at the
screen.

**Scope note (glyph atlas — `etagere` + `swash`, 4×1 subpixel — this phase's
next slice of deliverable 2, tracked in
`docs/receipts/phase-02-atlas-glifos-etagere-swash-4x1.md`)**: still no
capability row movement, same reasoning as above — this is the rasterization
and packing layer underneath "GPU text rendering", with no `wgpu::Texture`
upload, no quad/text render pipeline, no `parley`/`fontique` layout
integration, and no `TextRenderingMode` yet, so nothing reaches a screen.
What exists and is verified (`cargo test -p lux-render glyph_atlas::`):
`GlyphAtlas` packs real rasterized glyph bitmaps — via `swash::scale::Render`
with `Format::subpixel_bgra()`, hinting on — into three `etagere`-allocated
planes (`Monochrome`/`Polychrome`/`Subpixel`, matching `swash`'s own
`Content::Mask`/`Color`/`SubpixelMask`), with a `GlyphKey` lookup cache so a
repeated (font, glyph, size, subpixel-phase) request never rasterizes or
packs twice. The 4×1 subpixel design is verified concretely: the same glyph
rasterized at its 4 horizontal phase buckets lands in 4 distinct atlas
rectangles (real `swash` output, real Ahem test-font bytes, not a stub), and
a plane that runs out of room reports `None` rather than corrupting a
neighboring glyph's bitmap. Cache eviction and `wgpu` texture upload are
explicitly out of scope for this slice — later loops.

**Scope note (`parley` + `fontique` integration — this phase's next slice
of deliverable 3, tracked in
`docs/receipts/phase-02-integracion-parley-fontique-layout-ligaduras.md`)**:
still no capability row movement — this produces `Layout`/cluster data, not
pixels; nothing connects it to the glyph atlas or a render pass yet, so
still nothing reaches a screen. What exists and is verified (`cargo test -p
lux-render text_layout::`): `TextLayoutContext` shapes one line of text at
a time via a real `parley::Layout`, with a font registered directly from
bytes (`fontique::Collection::register_fonts`) rather than system
discovery — `fontique`'s `system` feature is off for a real, load-bearing
reason documented in `docs/DEPENDENCIES.md`: it pulls a `fontconfig` native
binding version that conflicts with the one `lux-platform`'s Wayland CSD
support already pins, and both crates share one workspace dependency graph.
System font enumeration (letting a user pick any installed font) is a
tracked gap, not a placeholder — it needs either dropping `crossfont`'s
conflicting pin or a native binding conflict resolution, neither of which
is this deliverable's job to decide. Verified with the same public-domain
Ahem test font the glyph atlas uses: exact uniform per-character advance on
a monospace grid, font-size scaling doubling the advance proportionally,
empty text producing zero clusters, and per-byte-range OpenType feature
overrides (`calt`/`liga`) accepted and applied without dropping the
line-default features outside that range — real per-run control, the thing
the master prompt asks for. **Not yet verified**: an actual visible
ligature collapse (Ahem defines no OpenType features to trigger one; needs
a real programming font, future work once one ships with the editor) and
IME preedit rendering (needs `lux-ui`, which doesn't exist yet).

**Scope note (`lux-ui`'s virtualized editor view — this phase's
deliverable 4, tracked in
`docs/receipts/phase-02-luxui-vista-editor-scroll-virtualizado.md`)**: the
"Virtualized editor view" row above stays `not-started` for the same
reason every other Phase 2 scope note gives — this produces the geometry a
render pass would draw (line layouts, gutter numbers, cursor/selection
rectangles, indent-guide and column-ruler x-positions), not pixels; the
`wgpu` quad/text pipeline that would paint it doesn't exist yet. What's
real and verified (`cargo test -p lux-ui --test golden_editor_view`, a
20-line real `Buffer` through a viewport that only fits 4 lines): only
those 4 lines are ever shaped — virtualization confirmed by construction,
not by inspection; line numbers right-aligned to the document's actual
digit width; a multi-cursor caret that blinks (a pure function of elapsed
time against a configurable interval) and is silently dropped from the
frame when scrolled off-screen rather than drawn somewhere wrong; a
multi-line selection rendered as one rectangle per visible line, with
correct partial-width rectangles on its start and end lines and full-width
rectangles in between; indentation guides derived from each line's real
leading whitespace and `Buffer::indent()`'s detected width; and column
rulers positioned via the same monospace shaping as the text itself, not a
separately-guessed character width. **Real gap found and deliberately
worked around, not silently papered over**: `lux-core::Buffer` has no
public per-line text accessor or `(line, column) -> Position` constructor
(only whole-document `text()` and a char-range `text_range()`); this view
builds its own byte-range line index from `buffer.text()` once per frame
(`editor_view::line_index`, private to `lux-ui`) rather than touching
`lux-core`, which was out of this deliverable's scope. That index build is
`O(document length)` per frame — fine at the sizes exercised here, a real
cost worth fixing with a proper `lux-core` per-line accessor before this
meets a large-file performance budget. **Not yet verified**: any of this
painted to an actual screen (needs the quad/text pipeline above), and
tab-expansion-aware indent guides (this pass counts every leading space or
tab as one column, matching `Position::column`'s current, not-yet-tab-aware
semantics — see the receipt).

**Scope note (`lux-platform`'s desktop IME — this phase's deliverable 5,
tracked in `docs/receipts/phase-02-ime-escritorio-preedit-area-cursor.md`)**:
the "Desktop IME (preedit, cursor area)" row above stays `not-started`
(macOS included) rather than moving to `implemented`, for two honest
reasons: only macOS was exercised (no Windows/Linux dev environment, same
gap `lux-platform`'s own entregable 1 already documented), and "preedit
visible en su sitio" in the literal sense the master prompt means — inline,
in the actual text position inside the editor — needs the `wgpu` text
pipeline, which doesn't exist yet; this pass showed live preedit text via
the window's title bar instead, an honest stand-in, not the real thing.
What's genuinely new and human-verified here, not just unit-tested:
`ImeComposition` (`lux-platform::ime`), a composition-state machine turning
`winit`'s raw `Enabled`/`Preedit`/`Commit`/`Disabled` sequence into
"here's the text being composed and its cursor" / "here's the final text,"
confirmed against **real macOS IME output**, not just assumptions — a
Spanish accent dead-key (´ then e) produced the exact sequence `Enabled` →
`Preedit("´", cursor_range=(2,2))` → `Preedit("", None)` (winit's own
synthetic clear) → `Commit("é")`, live, in a real window
(`cargo run --example ime_smoke -p lux-platform`), with the window title
updating to show the live preedit and the final commit. That live output
also independently confirmed something the automated tests could only
infer from `winit`'s docs: `cursor_range` is a **byte** offset, not a char
count — `(2, 2)` is exactly after "´", which is 2 bytes in UTF-8. **Not yet
verified**: any CJK IME (no Japanese/Chinese input source was available to
test with), and whether the OS actually anchors its own candidate window
at the position `set_ime_cursor_area` reports — a single dead-key accent
has no candidate list to show, so there was nothing positioned to observe;
that needs a CJK IME session, still open for a future pass on a machine
that has one.

**Scope note (`accesskit` conectado — this phase's deliverable 6, tracked
in `docs/receipts/phase-02-accesskit-conectado.md`)**: the "Accessibility
tree (`accesskit`)" row above stays `not-started` even though this pass's
manual verification genuinely succeeded end-to-end with a real screen
reader — the row tracks this wired into the actual, unified `lux` editor,
which doesn't exist yet (no `crates/lux` binary assembling
`lux-platform`+`lux-render`+`lux-ui` together), and ADR 0005's own
acceptance bar is VoiceOver **and** NVDA **and** Orca, of which only
VoiceOver could be exercised on this machine. What's real: `lux_ui::
accessibility::build_tree` (`cargo test -p lux-ui accessibility::`, 8
tests) builds an actual `accesskit` tree — a `MultilineTextInput` root with
one `TextRun` child per line, real per-character byte lengths/positions/
advances from `lux_render`'s own shaping, and a `TextSelection` tracking
the real primary cursor — confirmed against a **real accesskit_winit
adapter and real VoiceOver** (`cargo run --example accessibility_smoke -p
lux-ui`, project owner's own verification, verbatim: "Todo funcionó como
debería"): VoiceOver announced the control as a multi-line text field,
read its three real lines of content correctly, and tracked the caret
moving in response to real Left/Right arrow input backed by
`lux_core::Position::next_grapheme`/`prev_grapheme`. **Not yet verified**:
NVDA and Orca (no Windows/Linux environment available, same gap every
other Phase 2 deliverable this session has hit), and multi-cursor exposure
(only the primary cursor/selection is exposed — no accessibility API models
more than one caret, so the others are intentionally not represented, not
lost from the editor's own state).

**Scope note (Lux Dark theme — this phase's deliverable 7, tracked in
`docs/receipts/phase-02-tema-lux-dark.md`)**: the "Lux Dark theme" row
above stays `not-started` — this defines the color identity, not paint on
screen; there is still no quad/text render pipeline to apply it to (same
gap every other Phase 2 deliverable this session has hit). What's real
(`cargo test -p lux-ui theme::`, 7 tests): `lux_ui::theme::Theme::LUX_DARK`
carries v0's identity forward verbatim — indigo-black background
`#12121B`, golden-amber accent `#F0B442` — verified by parsing those exact
hex strings from `LUX_PROMPT_MAESTRO.md` and asserting they equal the
constant's fields byte-for-byte, not just eyeballed. `foreground`,
`selection`, and `muted` are this deliverable's own reasonable completions
(every theme needs a foreground; a translucent selection derived from
`accent`), not values pulled from any spec — flagged as such in the code,
not silently presented as part of the documented identity. "Golden capture"
here means a deterministic RGBA8 pixel buffer (`capture_swatch`) rendered
from the theme's real color values via flat fills (no antialiasing, so
exact byte comparison is the correct tolerance — zero), checked pixel-by-
pixel against the expected color at each swatch position. A full
token/scope-based theme system (Lux Light, high-contrast, per-syntax
tokens) is Phase 3's job, not this one's.

**Scope note (performance budgets — cold start, keypress latency, scroll —
tracked in
`docs/receipts/phase-02-presupuestos-rendimiento-arranque-latencia-scroll.md`)**:
none of Phase 2's three named budgets is fully measurable yet, for the
same reason as every other deliverable in this phase — no `wgpu` quad/text
pipeline and no unified `lux` binary exist to measure the literal, full
metrics against (a real cold start to first paint, a real keypress-to-
pixel latency, a real scrolling frame time). `crates/lux-render/benches/
phase2_budgets.rs` (`cargo bench -p lux-render -- --baseline none`)
measures the real CPU-bound component of each budget that exists today —
GPU context negotiation, line reshape + new-glyph rasterization, and
per-frame visible-line shaping — and all three land comfortably within
their share of the full budget (see `docs/PERFORMANCE.md`'s measurement
history for the numbers). That's a good sign, not proof the full budgets
are met; re-measure the real thing once the render pipeline and a unified
app exist.

**Update (quad/text pipeline, glyph cache eviction, `TextRenderingMode` —
the rest of deliverable 2, tracked in
`docs/receipts/phase-02-luxrender-pipeline-quads-texto-cache-textrenderingmode.md`)**:
the "GPU text rendering (glyph atlas, subpixel AA)" row above **still**
stays `not-started` — the reasoning that's kept every Phase 2 row
unmoved all session holds one more time: this is real, working, and
pixel-verified, but there is still no live window, no unified `lux`
binary, and no `lux-ui` wiring calling into any of it. What changed:
`lux-render` now has real `wgpu::RenderPipeline`s for solid quads and for
textured glyphs sampling all three atlas planes, `GlyphAtlas` now evicts
its least-recently-used entries instead of just refusing new glyphs once
a plane fills, and `TextRenderingMode` (`PlatformDefault`/`Subpixel`/
`Grayscale`) is threaded all the way through glyph rasterization. Most
importantly: `crates/lux-render/tests/golden_render.rs` renders a real
frame (background quad, a monochrome glyph, a subpixel glyph) to a real
offscreen `wgpu` texture on real macOS Metal hardware, reads the actual
pixels back, and asserts on them — the literal "Golden tests de render:
capturas de framebuffer comparadas con referencias, con tolerancia
definida" acceptance criterion, genuinely met for the first time this
project, on the one platform available. Subpixel compositing works
without dual-source blending, by mixing the mask against a known
background color directly in the fragment shader and outputting opaque —
correct specifically because it composites against a solid, known
background (see the pipeline's own doc comment for why that's an inherent
property of subpixel AA, not a shortcut). **Not yet verified**: a live
window showing real, readable text (the golden tests use the Ahem test
font — solid squares, deliberately, for exact pixel assertions; a
typographic quality check needs a real font with letterforms, which is
a `lux-ui` integration concern, not this crate's); Windows/Linux (same
gap as always); and the phase-level "texto indistinguible del nativo"
criterion, which needs that live, readable-font comparison.

**Update (render loop scheduler — deliverable 8, tracked in
`docs/receipts/phase-02-loop-render-presupuesto.md`)**: `lux_render::
schedule::RepaintScheduler` is real, tested, headless policy: no
`if now - last_paint > interval` polling anywhere (golden rule #5) —
a repaint is only ever due because something explicitly invalidated a
region, invalidated everything, or a previously-requested timed wakeup
(e.g. the next cursor-blink transition) has actually arrived. Small
invalidations coalesce into their union past a fixed region count instead
of growing unbounded. **Not yet wired to anything**: no event loop exists
to call it yet (`lux-platform`'s desktop backend doesn't drive a render
loop at all today) — this is the policy a future one will consult, not a
running scheduler.

**Closing update (deliverables 5 and 6, IME and `accesskit` — cross-
platform verification re-checked, see
`docs/receipts/phase-02-cierre-multiplataforma-ime-accesskit.md`)**: this
pass re-confirmed, directly rather than by assumption, that Windows/
Linux/NVDA/Orca verification is genuinely unreachable from this machine:
there is no `rustup` installed (Rust came from Homebrew, which ships only
the host `aarch64-apple-darwin` target's standard library — confirmed by
inspecting `rustc --print sysroot`'s `lib/rustlib` directory directly),
so there is no way to even `cargo check` against a Windows or Linux
target, let alone run NVDA or Orca. This matches and directly re-confirms
(not just repeats) `lux-platform`'s own Phase 2 entregable-1 conclusion.
Both deliverables' actual feature work (real IME composition tracking,
real `accesskit` tree building) was already complete and macOS-verified
in their own receipts; this closes the loop on the cross-platform half by
establishing, concretely, that nothing more is possible from this
session — not a gap left open by omission.

**Closing update (`crates/lux`, the first unified desktop binary, tracked
in `docs/receipts/phase-02-lux-app-binario-unificado.md`)**: every row
above now moves from `not-started` to `implemented` on macOS — the gap
every single prior Phase 2 note in this file cited ("no unified `lux`
binary exists yet") is closed. A real window opens, shows real live text
in a real system font (San Francisco Mono, read directly from
`/System/Library/Fonts/SFNSMono.ttf` — no bundled font ships yet, tracked
as a real gap), through the actual `wgpu` quad/text pipeline, `GlyphAtlas`,
and `EditorView` built across this phase's earlier deliverables. Real
keyboard-driven editing (typing, Backspace/Delete, Enter, Tab, arrow-key
and Home/End cursor movement, mouse-wheel scroll) mutates a real
`lux-core::Buffer`. Real inline IME composition renders at the actual
cursor screen position (not the window title bar, the earlier deliverable's
own honest stand-in) — confirmed with a live accented dead-key sequence.
Real `accesskit` tree updates are pushed through a new `lux-platform`
mechanism (`docs/adr/0013-platform-accessibility-adapter-wiring.md`) and
confirmed end-to-end with VoiceOver. Lux Dark's background/accent colors
paint the real window. All of this was confirmed directly by the project
owner on real hardware, not inferred: "todo funciona" across theme, cursor
movement, scroll, inline IME preedit, and VoiceOver, after one real bug
(`swash::FontRef`'s cache key regenerating every frame, defeating the
glyph atlas cache) was found and fixed mid-verification.

**Still not-started or out of scope, honestly**: Windows/Linux (same
`rustup`-less environment gap as every other Phase 2 deliverable) and
NVDA/Orca (no environment to run them); gutter line numbers are reserved
space but not yet painted as glyphs (a second shaping pass, deferred);
accessibility actions (`AccessibilityActionRequested`) are received but not
yet wired to real buffer mutation (screen readers can read content/
cursor/selection, not drive them); a lost `wgpu` surface isn't recovered
(would need rebuilding the surface from scratch, not implemented); and the
background color fed to the subpixel-compositing shader is sRGB-byte-
normalized rather than rigorously gamma-correct — internally consistent
(the clear quad and the uniform always agree), a real but minor polish
item, not a correctness bug hit today.

## Syntax & navigation (Phase 3, `v0.2.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Tree-sitter incremental highlighting | implemented | not-started | not-started | not-started | not-started |
| Token-based themes | implemented | not-started | not-started | not-started | not-started |
| Bracket pair colorization | implemented | not-started | not-started | not-started | not-started |
| Syntax-based code folding | implemented | not-started | not-started | not-started | not-started |
| Auto-indent by language | implemented | not-started | not-started | not-started | not-started |
| Goto Anything | not-started | not-started | not-started | not-started | not-started |
| Command palette | not-started | not-started | not-started | not-started | not-started |
| Search/replace bar | not-started | not-started | not-started | not-started | not-started |
| Tabs + status bar | not-started | not-started | not-started | not-started | not-started |
| Minimap | not-started | not-started | not-started | not-started | not-started |

**Tree-sitter incremental highlighting** is `lux-syntax`: incremental
parsing (`Document::edit` applies a `tree_sitter::InputEdit` to the previous
tree, then reparses so only the changed range's subtrees are rebuilt), a
background worker (`BackgroundHighlighter`, a dedicated thread driven by a
blocking channel receive, never polled), and highlight-query-based token
extraction (`Document::highlight`). All 18 grammars from
`docs/adr/0008-tree-sitter-grammars-compiled-in.md`'s fixed set are vendored,
compiled in, and pass a per-language golden token test: `cargo test
-p lux-syntax highlight:: --test golden_tokens`. **Note on the count**: ADR
0008's prose lists "17" comma-separated items, but TypeScript/TSX are two
distinct grammars (`tree_sitter_typescript`/`tree_sitter_tsx`), so the actual
compiled-in set is 18, not 17 — and not the "15" this deliverable's own
tracking name used either. See
`docs/receipts/phase-03-luxsyntax-treesitter-incremental-15-lenguajes.md`.
Verified on macOS only (no other dev environment available yet, same
constraint noted throughout this document). "Implemented" here is the
library capability, checked by golden tests — wiring the computed tokens
into `lux-ui`'s editor view (actual on-screen coloring) is separate work,
tracked under this phase's "Token-based themes" row above.

**Update, Phase 3 close-out:** `Document::highlight` scans the *whole*
tree on every call, which measured at ~16.8 ms for a reparse+highlight
after one keystroke in a 20k-line file — over the phase's 16 ms frame
budget. Added `Document::highlight_range(source, byte_range)`, scoped with
`tree_sitter::QueryCursor::set_byte_range` (not a post-hoc filter); a
simulated 50-line visible-range scope measures at ~2.3 ms. Also cached the
compiled highlight `Query` as a `Document` field instead of recompiling it
(`Query::new`, hundreds of lines of `.scm` patterns) on every call — the
dominant cost, it turned out, more than the tree traversal itself. See
`docs/PERFORMANCE.md`'s Phase 3 measurement history for the full numbers
and `benches/benches/phase3_budgets.rs`. `highlight` itself is unchanged
(now delegates to `highlight_range(source, 0..source.len())`, verified
byte-identical against the existing golden token tests) — a per-keystroke
UI should call `highlight_range` with the visible range once wiring
exists.

**Token-based themes** is `lux-syntax::theme`: a self-contained theme format
(background/foreground pair plus a scope-to-color table, the same shape as
a TextMate/VS Code theme) resolved with TextMate-style dotted-prefix
fallback — `"keyword.conditional.ternary"` colors as `"keyword"` if the
theme has no more specific entry, walking one `.segment` at a time down to
the theme's flat `default`. Three themes ship: `LUX_DARK` and `LUX_LIGHT`
(background/foreground pass WCAG AA, >= 4.5:1) and `LUX_HIGH_CONTRAST`
(passes AAA, >= 7:1 — measured by a real relative-luminance/contrast-ratio
calculation in the test, not eyeballed). `LUX_DARK`'s background/foreground
match `lux-ui::Theme::LUX_DARK` (Phase 2) verbatim for brand consistency,
duplicated by value rather than by a cross-crate dependency (`lux-syntax`
has no UI dependency, golden rule #2). Gate: `cargo test -p lux-syntax
theme::`. Verified on macOS only, same constraint as above. "Implemented"
here is again the library capability (three real, distinct color tables
covering every scope name the vendored grammars emit) — wiring a
`SyntaxTheme`'s colors into `lux-ui`'s actual rendering, and a theme picker,
are separate, not-started work. See
`docs/receipts/phase-03-temas-basados-tokens-scopes.md`.

**Bracket pair colorization** is `lux-syntax::brackets`: `bracket_pairs`
walks the real parse tree's leaf tokens (not a text scan, so a `(` inside a
string or comment is never mistaken for a real bracket) with a per-kind
stack, matching `(`/`)`, `[`/`]`, `{`/`}` and recording each pair's nesting
depth; `pair_at` finds "the pair under the cursor" for highlighting both
delimiters. Depth-to-color cycling lives on `theme::SyntaxTheme` (a new
`bracket_colors` cycling palette, one per shipped theme) rather than a
separate color system — reusing the theme infrastructure from the previous
deliverable instead of duplicating it. A real bug surfaced and got fixed
during this work: tree-sitter's error-recovery synthesizes zero-width
`MISSING` bracket nodes for unbalanced input, which the first version
wrongly treated as real closers, fabricating pairs that were never typed;
fixed by skipping `node.is_missing()` nodes.

**Syntax-based code folding** is `lux-syntax::folding`: `folding_ranges`
walks the same tree and treats every node spanning more than one source
line as foldable (deduplicated to the widest range per starting row, since
e.g. a function item and its block both start on the same line) — derived
entirely from real node spans, never from counting `{`/`[`/indentation
(the v0 mistake `LUX_PROMPT_MAESTRO.md` calls out by name for this exact
deliverable).

Gate for both: `cargo test -p lux-syntax -- folding:: brackets::` (note the
`--` before the two filters — `cargo test` only accepts one bare filter
argument directly; passing two space-separated filters needs the `--`
separator so they reach the test harness, which does support multiple
OR'd filters. The goal's literal command omitted it; see
`docs/receipts/phase-03-bracket-pair-colorization-folding-real.md` for the
full explanation). Verified on macOS only, same constraint as above.
"Implemented" is again the library capability — wiring bracket colors and
foldable gutter markers into `lux-ui`'s rendering is separate, not-started
work.

**Auto-indent by language** is `lux-syntax::indent_rules`: `indent_level`
counts how many of a language's body-node kinds (`block`,
`compound_statement`, `object`, ... — one set per grammar, established from
each grammar's real parse tree, the same way `highlights.scm` establishes
each grammar's capture names) enclose the target line — never by counting
`{([:` in the source text (the v0 mistake `LUX_PROMPT_MAESTRO.md` calls out
by name for this exact deliverable), so a brace inside a string or comment
never fools it. Two corrections on top of the raw count: a line whose first
token is itself the *last* token of its innermost enclosing body (a closing
`}`, a `fi`, a `done`) dedents to the level of the line that opened that
body, rather than counting as inside it; and YAML/Markdown subtract one
level for the document's own top-level wrapper, since both grammars reuse
the same node kind (`block_mapping`/`block_sequence`, `list`) for genuine
nesting and for "the whole file," unlike JSON/HTML's outermost node, which
has a real opening delimiter and needs no such correction. Gate: `cargo test
-p lux-syntax indent_rules::`. Verified on macOS only, same constraint as
above. "Implemented" is again the library capability — wiring computed
indent into an actual Enter-key/reindent editor command is separate,
not-started work; no editing UI exists yet at this phase for `lux-syntax`
to depend on (golden rule #2). See
`docs/receipts/phase-03-autoindent-lenguaje.md`.

**Goto Anything** is still `not-started` as a user-facing feature — there is
no `Cmd+P` command, no dialog, and no real project file index to search
(that's `lux-workspace`'s incremental index, Phase 4, not built yet). What
does exist, as groundwork: `lux-search::fuzzy::rank_paths` wraps `nucleo`
(`docs/adr/0009-search-and-fuzzy-matching.md`) with a `Config::match_paths`
tuning and a set of characterization tests pinning its ranking behavior, per
that ADR's own instruction; and `lux-ui::goto_anything` parses the
Sublime-style `file:line` suffix (`src/main.rs:42`) and combines it with
that ranking. Gate: `cargo test -p lux-ui goto_anything::`. Verified on
macOS only, same constraint as above. See
`docs/receipts/phase-03-goto-anything-nucleo-sufijos-ficherolinea.md`.

**Update, Phase 3 close-out:** recent-file history and preview — the other
two things this deliverable's `LUX_PROMPT_MAESTRO.md` sentence names — are
now implemented too: `RecentFiles` tracks opened files with a logical
clock (same technique as `command_palette::CommandRegistry`'s recency),
and `search_with_recency` lists recents first for an empty query, then
ranks by fuzzy score with recency as a tie-break, same as the command
palette's own search. `preview` returns a text snippet around a match's
target line, from an already-loaded `Buffer` (never opens a file itself —
that's I/O, which stays off the UI thread, golden rule #6). `@symbol` and
`#text` are **not** implemented, and deliberately so, not just
unfinished: they need capabilities Phase 4 (`lux-search`'s Find in Files)
and Phase 7 (`lux-lsp`) own, and building throwaway versions now would
either duplicate those phases' work or pull them forward against golden
rule #7. See `docs/adr/0014-goto-anything-symbol-text-suffixes-deferred.md`
for the full reasoning and what unblocks each one.

**Command palette** is also still `not-started` as a user-facing feature —
no `Cmd+Shift+P` dialog, and no menu UI exists yet in `lux-ui` for it to
unify with (Phase 3 has shipped no interactive widgets so far, only passive
ones). What does exist: `lux-ui::command_palette::CommandRegistry`, a single
registry (`register`/`commands`/`search`) that is structurally the only
place a command can be defined — there is no separate "menu item" type
anywhere in the crate, so a future menu has no list to source itself from
other than this one, which is what makes "every menu command is in the
palette" hold rather than just being a rule someone has to remember (v0's
actual failure, named in this deliverable in `LUX_PROMPT_MAESTRO.md`).
Ranking reuses the same `nucleo` engine as Goto Anything through a new
`lux-search::fuzzy::rank_by` (generic over any item + key function, unlike
`rank_paths`, because two commands can share a display title). Recency is
tracked with a logical clock, not a wall-clock timestamp — deterministic
and testable, and only relative order is ever needed. `when` clauses are a
real small parser (`&&`/`||`/`!`/`(...)`, standard precedence) over boolean
context keys, not a stub — malformed clauses are a parse error, not a
silently-ignored one. Gate: `cargo test -p lux-ui command_palette::`.
Verified on macOS only, same constraint as above. See
`docs/receipts/phase-03-paleta-comandos-unificada-menus.md`.

**Search/replace bar** is also still `not-started` as a user-facing
feature — no `Cmd+F`/`Cmd+H` dialog, and no highlight rendering pass in
`EditorView` (Phase 3 has shipped no interactive widgets so far). The
search/replace engine itself — literal, case-insensitive, whole-word, and
regex, with capture-group replacement — was already `implemented` in Phase
1 as `lux-core::find_replace` (this doc's "Find/replace in buffer" row);
this deliverable adds no new matching logic. What's new is
`lux-ui::search_bar::SearchBar`, the stateful part a bar UI needs on top
of that engine: `matches()` (every match, for "highlight all of them"),
`current()`/`current_index()` (which one is active, for distinct
highlighting), `next_match()`/`previous_match()` (wrapping navigation by
index over the already-computed list — cheap, since the full list is
already held for highlighting, so navigating doesn't re-run the regex),
and `replace_current()`/`replace_all()` (delegating straight to
`lux-core::find_replace`, then refreshing). An invalid regex while typing
leaves the previous good query and its matches in place instead of
blanking the bar, the same "don't discard input on a transient invalid
state" choice made for Goto Anything's `:line` suffix parsing. Gate:
`cargo test -p lux-ui search_bar::`. Verified on macOS only, same
constraint as above. See
`docs/receipts/phase-03-barra-busqueda-reemplazo-resaltado-coincidencias.md`.

**Tabs + status bar** is also still `not-started` as a user-facing feature
— no actual tab strip or status bar widget renders anything, and no mouse/
drag event handling exists (Phase 3 has shipped no interactive widgets so
far). What exists: `lux-ui::tabs::TabBar`, the state machine a tab strip
would drive — open/close, reopening the most recently closed tab (a LIFO
stack), pinning (pinned tabs cluster at the start of the strip, in the
order they were pinned), and `move_tab` (the reorder-by-index primitive a
drag & drop gesture reports its drop position through — the actual pointer
tracking is separate, not-started work). Tab ids are minted by `TabBar`
itself, never caller-chosen, so two tabs can't collide by construction.
And `lux-ui::status_bar`, which formats cursor position and the
already-Phase-1-detected encoding/EOL/indent (`lux-core::buffer`, this
doc's "EOL / indent / encoding detection" row) for display; language is a
caller-supplied label since language detection isn't wired into `lux-ui`
yet. "All clickable" is enforced the same way `command_palette` enforces
"every menu command is in the palette": a `StatusBarItem` has no
constructor that omits a `command_id`, and `status_bar::register_commands`
registers those exact ids into a real `CommandRegistry` — a status bar
item's click target is always a real, registered command, checked by a
test (`every_built_status_bar_item_resolves_to_a_registered_command`), not
just a string sitting next to a label. Gate: `cargo test -p lux-ui
tabs:: status_bar::` (needs `--` before the two filters, same reason as
`docs/receipts/phase-03-bracket-pair-colorization-folding-real.md`:
`cargo test` takes one bare filter, so `cargo test -p lux-ui -- tabs::
status_bar::` is what actually runs both). Verified on macOS only, same
constraint as above. See `docs/receipts/phase-03-pestanas-barra-estado.md`.

**Minimap** is also still `not-started` as a user-facing feature — nothing
renders, no click-to-scroll is wired to real input (Phase 3 has shipped no
interactive widgets so far). What exists: `lux-ui::minimap`, which —
deliberately — never shapes a single glyph. `EditorView` (Phase 2
deliverable 4) already only shapes visible lines specifically so a huge
document stays fast; a minimap that shaped real text for all 50k lines of
that same document every frame would quietly reintroduce the exact cost
virtualization exists to avoid, just relabeled. Instead each source line
reduces to a density bar (indent offset + content length, both plain
character counts via the same `LineIndex` byte-splitter `EditorView`
already uses — no new dependency on `lux-render`'s shaping at all). When
the document has more lines than the minimap has pixel rows for,
`build_frame` downsamples: several source lines collapse into one row,
represented by the *densest* line in that group rather than an average, so
a long line is never washed out by its blank neighbors — and the row count
this produces is only ever what the content actually needs (no padding to
`available_rows` with empty trailing rows when it doesn't divide evenly).
A `ViewportIndicator` translates the main editor's visible line range into
minimap rows through that same scale, and `line_for_row` is the inverse —
the click-to-scroll navigation a minimap's whole interactive point is,
here as a pure function ready for real input wiring later. `enabled`
defaults to `true` (every editor that has a minimap ships it on); a
disabled minimap's `build_frame` returns `None`, not an empty frame
(golden rule #1). Gate: `cargo test -p lux-ui minimap::`. Verified on
macOS only, same constraint as above. See
`docs/receipts/phase-03-minimapa.md`.

## Projects, files & search (Phase 4, `v0.3.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Multi-root workspaces | not-started | not-started | not-started | not-started | not-started |
| File tree explorer | not-started | not-started | not-started | not-started | not-started |
| Native FS watchers | not-started | not-started | not-started | not-started | not-started |
| Find in files (ripgrep engine) | not-started | not-started | not-started | not-started | not-started |
| Replace in project with diff preview | not-started | not-started | not-started | not-started | not-started |
| Sessions & crash recovery | not-started | not-started | not-started | not-started | not-started |
| Huge/binary file handling | not-started | not-started | not-started | not-started | not-started |

**Scope note (`lux-workspace`'s multi-root data model + `.lux-project` file —
this phase's first deliverable, tracked in
`docs/receipts/phase-04-luxworkspace-multiroot-luxproject.md`)**: no
capability row above moves yet — this is the foundational data model other
Phase 4 deliverables (file tree, watchers, find in files, the incremental
index) will key their own per-root state off, not a user-visible capability
by itself; there's still no explorer, no watcher, no search wired to any of
it, and nothing wired into the `lux` binary. What exists and is verified:
`Workspace` (ordered, independently-addressable roots via `RootId`,
add/remove, longest-prefix `root_for_path` resolution for scoping later
search/commands to one root) and `ProjectFile` (the `.lux-project` on-disk
schema — `folders` with optional per-folder display names, `excludes`
glob-pattern strings stored opaquely, and an untyped `settings` bag deferred
to `lux-config` in Phase 5 — load/save as plain JSON via `serde`/
`serde_json`, see ADR 0015), plus `Workspace::discover` tying the two
together (loads `.lux-project` when a folder has one, single-root fallback
otherwise). Verified on macOS (aarch64-apple-darwin):
`cargo test -p lux-workspace project::` (21 tests, all headless — no UI
dependency per golden rule #2).

**Scope note (`lux-ui`'s virtualized file tree — this phase's second
deliverable, tracked in
`docs/receipts/phase-04-explorador-arbol-virtualizado.md`)**: the "File tree
explorer" row above stays `not-started` — this is the state/data layer a
tree view widget would render and drive (same pattern as every other
`lux-ui` deliverable so far: `tabs`, `command_palette`, `minimap`, ...), not
pixels or mouse/keyboard/drag event handling, none of which exist yet. What
exists and is verified: `FileTree`, with lazy per-directory loading (a
directory's entries are read from disk only when expanded, never eagerly —
the real fix for v0's flat, 400-item-capped explorer list) feeding a
flattened `rows()`/`visible_rows(first, count)` pair a virtualized list
slices by scroll position instead of ever materializing the whole tree; and
real (not simulated) filesystem operations — `create`, `rename` (updating
every already-loaded descendant's path), `move_node` (the operation a drag
& drop gesture reports its drop through, backed by a real cross-directory
`fs::rename`, unlike `tabs::TabBar::move_tab`'s in-memory-only reorder,
since here the files are real), `delete_to_trash` (a real move to the
system trash via the `trash` crate, never a permanent delete), and `reveal`
(expands a path's ancestors so it lands in `rows()`). Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-ui file_tree::` (19 tests,
including one that performs a real system-trash deletion). **Not yet
exercised**: Windows'/Linux's trash backends (`trash` 5.2.6 supports both,
per its own docs, but only macOS has been run here); any real mouse/drag
event, since no interactive UI exists in `lux-ui` yet. Whichever layer
eventually wires this into the running app must dispatch these operations
off the UI thread (golden rule #6) — that wiring is separate, not-started
work.

**Scope note (`lux-workspace`'s native watchers — this phase's third
deliverable, tracked in
`docs/receipts/phase-04-watchers-nativos-notify-debouncer.md`)**: the
"Native FS watchers" row above stays `not-started` — `FileWatcher` is
verified directly against the real OS backend (not mocked), but nothing
consumes its output yet: no file tree auto-refresh, no editor reload
prompt, nothing wired into the `lux` binary. What exists and is verified:
`notify` 8.2.0 + `notify-debouncer-full` 0.7.0 (the version that actually
pairs with `notify` 8.2.0, not the crate registry's default "latest," which
tracks `notify` 9's still-unstable RC — see `docs/DEPENDENCIES.md`), fully
event-driven (the debouncer thread blocks on FSEvents/inotify/
`ReadDirectoryChangesW`, no timer anywhere per golden rule #5), classifying
each change as `Created`/`Removed`/`Reload` (no local unsaved edits, safe
to silently reload)/`Conflict` (the watched path is in the caller-driven
`mark_dirty` set — no buffer/document manager exists yet to ask
automatically, so this is deliberately caller-driven); Linux's inotify
watch-limit error (`notify::ErrorKind::MaxFilesWatch`) surfaced distinctly
via `is_watch_limit_reached` rather than swallowed into a generic error.
**Two real macOS FSEvents particularities were found and handled by testing
against the live backend, not by assuming the docs**: (1) editing a file
that already existed before `watch()` was called is reported as another
`Create`, not a `Modify`, confirmed to persist well past the debounce
window — `watch()` now seeds a `known`-paths set from a one-time recursive
scan of the root so `classify` can tell the two cases apart; (2) deleting a
file shortly after creating it (even across separate, real syscalls) makes
FSEvents coalesce the whole lifecycle into one raw batch, which trips
`notify-debouncer-full`'s own create-then-remove cancellation heuristic and
silently swallows the real `Remove`, leaving only stray `Modify` events —
`classify` now treats any event whose path no longer exists on disk as
`Removed` regardless of the kind the debouncer attached. Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-workspace watcher::` (10 tests,
6 of them driving the real FSEvents backend against real temp
directories — re-run 3× clean to rule out flakiness from the timing
involved). **Not yet exercised**: Linux's inotify backend and Windows'
`ReadDirectoryChangesW` backend (no non-macOS dev environment available —
same documented limitation as the rest of this file); the `MaxFilesWatch`
limit itself (tested only via a synthetically constructed `notify::Error`,
not a real 8-million-watch exhaustion, which isn't practical in a unit
test). Whichever layer eventually wires this into the running app must call
`watch()`/drain the channel off the UI thread (golden rule #6) — that
wiring is separate, not-started work.

**Scope note (`lux-search`'s Find in Files — this phase's fourth
deliverable, tracked in
`docs/receipts/phase-04-find-in-files-motor-ripgrep.md`)**: the "Find in
files (ripgrep engine)" row above stays `not-started` — no dedicated,
navigable results panel exists in `lux-ui` yet, and nothing is wired into
the `lux` binary. What exists and is verified: `find_in_files`, the real
embedded ripgrep engine (`ignore` for `.gitignore`-aware, glob-filterable
traversal via a real `WalkParallel` fanning the search out across threads;
`grep-searcher` + `grep-regex` for the actual line-oriented regex search —
not a hand-rolled walker with `String::find`), streaming matches back
through a channel as they're found (`SearchStream::poll`/`wait_until_done`)
and staying navigable (`next_match`/`previous_match`/`current`) the same
way `lux_ui::search_bar::SearchBar` already is for in-buffer search.
`.gitignore` is honored even outside an actual git repository
(`require_git(false)`, a deliberate choice — see the receipt for why the
`ignore` crate's own default would otherwise silently skip it for a
project folder that isn't `git init`-ed yet); include/exclude globs and
case-insensitive matching are real options, not stubs; a binary or
non-UTF-8 file is skipped, not fatal to the whole search. Verified on
macOS (aarch64-apple-darwin): `cargo test -p lux-search find_in_files::`
(11 tests) plus the named performance budget, measured for real —
`cargo bench -p lux-benches --bench phase4_budgets`: **43.6 ms** (mean, 10
samples) against a real ~1.07 GB on-disk corpus (2,000 files, 30,000 real
matches), against the < 2 s budget — see `docs/PERFORMANCE.md`'s
measurement history for the full corpus description and the caveats
(parallelism across cores, page-cache warmth on repeated samples) that
number carries. **Not yet exercised**: the dedicated results panel itself
(no interactive UI exists in `lux-ui` yet); Windows/Linux (no non-macOS
dev environment available — same documented limitation as the rest of
this file).

**Scope note (`lux-search`'s replace-in-project preview — this phase's
fifth deliverable, tracked in
`docs/receipts/phase-04-replace-proyecto-preview-diff.md`)**: the "Replace
in project with diff preview" row above stays `not-started` — no diff UI
or confirmation dialog exists in `lux-ui`, nothing is wired into the `lux`
binary. What exists and is verified: `build_preview`, a pure function over
an already-run `find_in_files` search's matches that computes, per match,
a before/after line pair (regex capture-group expansion, same `$1`/
`${name}` syntax `lux_core::find_replace` already uses for in-buffer
replace) — no disk I/O of its own. Every computed hunk starts unaccepted;
`ReplacementPreview::accept`/`accept_all_in_file` are the only way to opt
one in (per-hunk or per-file confirmation, as the deliverable names), and
`apply` only ever writes hunks that were explicitly accepted — never a
blind mass replace, per the master prompt's own words. `apply` also
re-checks each accepted hunk's original line against the file's *current*
content immediately before writing it, reporting a per-hunk `conflicted`
result instead of silently overwriting a file that changed on disk since
the preview was built. Verified on macOS (aarch64-apple-darwin):
`cargo test -p lux-search replace_preview::` (13 tests, including a real
on-disk conflict-detection case and CRLF-preservation). **Not yet
exercised**: the diff/confirmation UI itself (no interactive UI exists in
`lux-ui` yet); Windows/Linux (no non-macOS dev environment available).

**Scope note (`lux-workspace`'s incremental workspace index — this
phase's sixth deliverable, tracked in
`docs/receipts/phase-04-indice-incremental-alimentado-watchers.md`)**: no
capability row above moves for this one either — like the multi-root data
model from deliverable 1, `WorkspaceIndex` is infrastructure a future
Goto Anything/Find in Files integration would key off (a live file list
instead of a fresh `ignore::WalkParallel` walk per search), not a
user-visible capability by itself, and nothing consumes it yet. What
exists and is verified: `WorkspaceIndex::build` does exactly one real,
`.gitignore`-aware recursive scan to establish the baseline file set;
every update after that goes through `apply_changes`, a plain function
over a batch of `lux_workspace::watcher::FileChange`s — no second scan,
ever, satisfying "fed by watchers, not periodic rescans" literally, not
just in spirit (golden rule #5). `Created`/`Reload` add a path only if
it's currently a real file (a directory create isn't indexed);
`Removed` drops it unconditionally (a harmless no-op if it was never
indexed); `Conflict` never changes membership. Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-workspace index::` (10 tests,
9 pure logic against a real on-disk baseline plus one genuine end-to-end
case — a real `FileWatcher` observes a real file creation and the index
is fed its actual output, not a synthetic `FileChange` — re-run 3× clean
to rule out timing flakiness). **Not yet exercised**: any real consumer
(Goto Anything, Find in Files) actually querying the index instead of
walking fresh each time — that rewiring is separate, not-started work;
Windows/Linux (no non-macOS dev environment available).

**Scope note (`lux-workspace`'s sessions and recovery — this phase's
seventh deliverable, tracked in
`docs/receipts/phase-04-sesiones-recuperacion.md`)**: the "Sessions &
crash recovery" row above stays `not-started` — no tab strip, cursor, or
scroll state exists anywhere real to persist yet (`lux-ui::tabs`'s
`TabBar` doesn't track cursors/scroll itself, and no unified `lux` binary
wires a real editor session to any of this), so this is the
persistence/recovery mechanism such a session manager would call into.
What exists and is verified: `Session` (folders, tabs with pinned/
cursors/scroll-line, active tab — `serde_json`, same convention
`project::ProjectFile` already established) round-trips through
`save`/`load`, and `Session::to_workspace()` rebuilds a real multi-root
`project::Workspace` from the saved folders — composing with deliverable
1's model instead of duplicating it, the concrete form "restore folders on
open" takes here. Crash detection is a real, testable protocol, not just a
flag: a session is written with `clean_shutdown = false` while running and
`= true` only as the last step of a normal quit (`end_cleanly`); a
`Session` loaded with `clean_shutdown` still `false` means the previous
run never got there — `needs_recovery()` — the same "explicit signal
survives past whatever crashed" idea a `.lock`/PID file gives, without
needing a separate lock file. `RecoveryStore` persists actual unsaved
buffer *content* per path (hashed to a filename so arbitrary paths never
need escaping logic) — `save` on every real edit event, `clear` after a
real save-to-disk succeeds, `recoverable()` listing what a "recover
unsaved changes?" prompt would show. Both types are caller-driven, the
same shape `watcher::FileWatcher::mark_dirty` already uses: nothing here
runs on a timer (golden rule #5). Verified on macOS (aarch64-apple-darwin):
`cargo test -p lux-workspace session::` (12 tests, all headless — no UI
dependency, no real watcher/OS backend needed for this deliverable's own
logic). **Not yet exercised**: any real editor session actually calling
`Session::save`/`RecoveryStore::save` on real tab/cursor/edit events (no
such session exists yet); a real crash scenario end-to-end (the protocol
is tested via the same `save`-without-`end_cleanly` sequence a crash would
leave behind, not an actual process kill); Windows/Linux (no non-macOS dev
environment available).

**Scope note (`lux-core`'s huge-file and binary-file handling — this
phase's eighth and last deliverable, tracked in
`docs/receipts/phase-04-ficheros-enormes-binarios.md`)**: the "Huge/binary
file handling" row above stays `not-started`, because the deliverable's
two halves are at genuinely different levels of completeness and rule 1
doesn't let a partial one round up. **Binary detection is fully real and
complete**: `binary_detect::looks_binary_path` reads only a small prefix
(not the whole file) to decide, and `Buffer::open_with_policy` refuses a
binary file with that same cheap check before ever attempting to decode
it as text — "detected and not opened as text," literally. **Huge-file
handling is a real, correct classification with deferred enforcement, not
a full read-only mode yet**: `huge_file::classify_path` cheaply checks a
file's size (`fs::metadata` only) against a configurable
`HugeFilePolicy`, and `Buffer::open_with_policy` sets a real, queryable
`is_huge_file_mode()` flag — but nothing in `lux-core` currently *enforces*
read-only (a deliberate choice: making `Buffer::insert`/`delete` silently
no-op would desync `Edit`/`Transaction`/`History`'s invertibility
invariants, a real correctness hazard in code this deliverable has no
reason to touch), and "no syntax highlighting" is entirely `lux-syntax`/
`lux-ui`'s domain, untouched here. The flag is real; a future editor-
session layer enforcing it against real edit commands and real
highlighting calls is what would actually move this row. Verified on
macOS (aarch64-apple-darwin): `cargo test -p lux-core huge_file::
binary_detect::` (15 tests) plus 6 new integration tests in
`buffer::tests` exercising `Buffer::open_with_policy` against real files
(a real huge file, a real small file, a real binary file larger than the
huge-file threshold — proving the binary check isn't skipped on the
huge-file path — and a configurable threshold changing classification for
the same file). **Not yet exercised**: any real enforcement of read-only
against edit commands or highlighting (no such consuming layer exists);
the "open a 1 GB file without exhausting memory" acceptance criterion as
its own measurement — `Buffer::open_with_policy` still loads a huge
file's full content through the same `fs::read`+decode path `Buffer::open`
already uses (there's no partial/virtualized load in this rope-backed
`Buffer`), so this deliverable doesn't change memory behavior for actually
opening a huge file — the ~3.16× RSS overhead documented in Phase 1's
measurement history for a 100 MB file is unchanged and still open, not
newly introduced or newly fixed here; Windows/Linux (no non-macOS dev
environment available).

**Phase 4 close-out.** All eight deliverables above have individual
receipts under `docs/receipts/phase-04-*.md`, each `Result: passed`
against its own gate, and the full workspace gate (`cargo test
--workspace`, `cargo clippy --workspace -- -D warnings`, `cargo deny
check`, `scripts/check-limits.sh`, `scripts/check-core-isolation.sh`)
passes clean at 592 tests, 0 failures. Two things worth stating plainly
rather than leaving implicit:

1. **No capability row above moved to `implemented`.** Every one of
   Phase 4's eight pieces is real, tested, and correct at the crate
   level — but none of it is wired into `crates/lux` (the real, running
   desktop binary confirmed working end-to-end at Phase 2's close), which
   today still only opens one fixed file with no project/folder concept
   at all. This is a deliberate, consistent choice made identically
   across all eight loops this phase (see each deliverable's own "Scope"
   note above), not an oversight discovered at close time. Phase 4 built
   the complete backend/engine layer for projects, files, and search;
   none of it is yet a capability an actual user of the running app has.
2. **Only one of the phase's four acceptance criteria is backed by a real
   measurement.** "Find in files over 1 GB of code, < 2 s" — measured,
   **43.6 ms**, see `docs/PERFORMANCE.md`. The other three were never
   literally measured, only architecturally argued from the design (the
   same distinction rule 3 in `CLAUDE.md` exists to prevent):
   - *"Opening a 100k-file project never blocks the UI"* — no test opened
     a real 100k-file tree; the claim rests on `lux_ui::file_tree`'s lazy
     per-directory loading never doing a recursive walk on open, and
     `lux_workspace::index`'s one-time baseline scan. Plausible, not
     measured.
   - *"10,000 file changes in 10 seconds doesn't degrade frame time"* —
     no stress test exists, and frame time has no real measurement
     surface yet: `crates/lux`'s render loop isn't wired to any watcher
     or index from this phase. Not measurable with the code as it stands
     today, not just unmeasured.
   - *"Opening a 1 GB file doesn't exhaust memory"* — never measured at
     1 GB; Phase 1 measured 100 MB and found (and left open, tracked)
     a ~3.16× RSS overhead. `Buffer::open_with_policy` (this phase's
     huge-file work) doesn't change that memory path at all — it adds
     cheap pre-checks before opening, not a different way of loading
     content once opening is decided.

   This is the same category of gap Phase 1 shipped with (the 100 MB RSS
   overrun) — documented and tracked rather than hidden, not blocking the
   phase, per the project owner's explicit decision when closing Phase 4.

## Configuration & personalization (Phase 5, `v0.4.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| JSONC config with layered precedence | not-started | not-started | not-started | not-started | not-started |
| Keymap presets (Sublime/VS Code/JetBrains) | not-started | not-started | not-started | not-started | not-started |
| `.editorconfig` / `.gitattributes` | not-started | not-started | not-started | not-started | not-started |
| Settings importers | not-started | not-started | not-started | not-started | not-started |
| Font settings + overrides | not-started | not-started | not-started | not-started | not-started |
| Schema-aware settings-file completion | not-started | not-started | not-started | not-started | not-started |

**Scope note (`lux-config::layering` — JSONC, layered precedence, hot
reload; the narrower first slice of this phase's `lux-config` deliverable,
tracked in
`docs/receipts/phase-05-luxconfig-jsonc-precedencia-capas-recarga.md`)**:
the "JSONC config with layered precedence" row above doesn't move — this is
a headless settings-resolution library with no schema, no keymaps, no
`.editorconfig`, no importers, and no preferences surface built on it yet,
and nothing reads or writes real settings through it from the `lux` binary.
What exists and is verified: `LayeredConfig` (JSONC parsing via
`jsonc-parser`, five layers — `Default`/`User`/`Project`/`Folder`/`Language`
— merged in that precedence order via a recursive JSON-object merge, a
malformed layer keeping its last good value instead of dropping settings or
the whole config) and `ConfigWatcher` (background `notify`-based hot reload
of every file-backed layer, no polling). Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-config layering::` (19 tests,
headless — no UI dependency per golden rule #2). Schema publication and
validation with line/column errors (the rest of this phase's first
deliverable bullet) landed in a later loop, once `crate::schema` existed
for a validator to check against — see the closing scope note below.

**Scope note (`lux-config::keymap` — keymaps as data, tracked in
`docs/receipts/phase-05-keymaps-sublime-vscode-jetbrains-como.md`)**: the
"Keymap presets (Sublime/VS Code/JetBrains)" row above doesn't move either
— same reasoning as the row above it: no key-event dispatch layer exists
yet for any of this to actually drive, and nothing is wired into the `lux`
binary. What exists and is verified: `Chord`/`ChordSequence` (parsed from
the same `"ctrl+shift+p"`/`"ctrl+k ctrl+s"` string convention VS Code's own
`keybindings.json` uses), `KeyBinding`/`Keymap` with per-platform
(`mac`/`win`/`linux`) resolution mirroring that same real schema, conflict
detection with a human-readable diagnostic, and three preset keymaps
(`crates/lux-config/keymaps/*.jsonc`) loaded through the same JSONC parser
`layering` uses. Verified on macOS (aarch64-apple-darwin): `cargo test -p
lux-config keymap::` (36 unit tests) and `cargo test -p lux-config
keymap:: --test keymap_presets` (7 tests checking each preset's documented
shortcuts, platform resolution, chord chaining via two of VS Code's real
`ctrl+k`-prefixed bindings, and conflict detection). Not yet built:
conflict resolution *policy* (this deliverable detects and reports
conflicts; deciding which binding wins, or prompting a user, is
unaddressed), and the two remaining phase-acceptance criteria this
deliverable doesn't cover on its own — key-chaining is supported by the
data model but real dispatch of a multi-chord sequence needs an input
layer that doesn't exist yet.

**Scope note (`lux-config::editorconfig` + `lux-config::gitattributes`,
tracked in `docs/receipts/phase-05-editorconfig-gitattributes.md`)**: the
"`.editorconfig` / `.gitattributes`" row above doesn't move either — same
reasoning as the two rows above it: nothing in `lux-core`'s buffer/save
path actually consults either of these yet (no auto-indent-detection
override, no EOL-normalization-on-save), and nothing is wired into the
`lux` binary. What exists and is verified: `editorconfig::resolve` (a
typed wrapper around `ec4rs`, a dedicated spec-compliant EditorConfig
implementation — real glob matching including brace alternation, real
`[section]` parsing, real directory-tree inheritance with `root = true`
stopping the upward walk, a malformed file surfacing a real, actionable
error rather than silently producing wrong data) and
`gitattributes::GitAttributes` (built on `gix-attributes`, the same
gitoxide-family crate `lux-git` will use for real Git integration in Phase
6 — narrowed to line-ending normalization per this deliverable's own
wording, resolving `text`/`eol`/the built-in `binary` macro into
`Normalize(Lf|CrLf)`/`Auto`/`Disabled`/`Unspecified`, with a later-added
file's rules correctly overriding an earlier one's for the same pattern).
Verified on macOS (aarch64-apple-darwin): `cargo test -p lux-config
editorconfig::` (9 tests) and `cargo test -p lux-config gitattributes::` (9
tests). Not built: full `.gitattributes` semantics beyond line endings
(diff/merge drivers, filters, `export-ignore`, `linguist-*` — out of scope
per the deliverable's own "para normalización de finales de línea"
wording); anything consuming either module to actually change editor
behavior (indent detection, EOL conversion on save).

**Scope note (`lux-config::import` — schema-aware settings importers,
tracked in `docs/receipts/phase-05-importadores-ajustes-externos.md`)**:
the "Settings importers" row above doesn't move either — same reasoning as
every row above it: nothing consumes an `ImportReport` yet (no "Import
from..." command, no UI, no write path into `layering::LayeredConfig`),
and nothing is wired into the `lux` binary. What exists and is verified:
three importers (`import::sublime`, `import::vscode`, `import::jetbrains`)
each mapping a modest, real subset of their source's setting keys onto
Lux's own dotted-key settings namespace, sharing one `ImportReport` shape
(`imported`/`ignored`, the latter split into `UnknownKey` vs a recognized
key with an `UnsupportedValue` — two different, separately-reported
outcomes, not one catch-all "ignored" bucket) — the "informen
honestamente" half of this deliverable's wording. Sublime's and VS Code's
importers reuse `layering::jsonc::parse` (both formats are JSON with
comments); JetBrains' reads XML via `quick-xml`. Every mapped source key
was either confirmed against the vendor's own documentation or source
(VS Code: `code.visualstudio.com`'s docs directly; JetBrains: read
straight from `CommonCodeStyleSettings.java` in the `intellij-community`
GitHub repo, the actual source that produces the XML) or, where that
wasn't reachable (most of Sublime's keys — its docs don't publish a plain
settings reference, same gap the keymap-preset loop hit for its
shortcuts), explicitly flagged as coming from well-established prior
knowledge instead of presented as uniformly verified. Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-config import::` (20 tests).
Not built: a preferences surface or any UI to trigger an import (Phase 5
deliverable 6, still not-started); writing an `ImportReport`'s imported
settings into a real `LayeredConfig` layer (this deliverable produces the
report; consuming it is a future integration's job, same design language
`layering`/`keymap`/`editorconfig`/`gitattributes` already established
this phase); import of anything beyond the handful of indent/whitespace/
font settings each importer currently recognizes.

**Scope note (`lux-config::font_settings` — typography with project/folder
overrides, tracked in
`docs/receipts/phase-05-ajustes-tipografia-overrides-proyecto-carpeta.md`)**:
the "Font settings + overrides" row above doesn't move either — same
reasoning as every row above it: nothing renders text through a resolved
`FontSettings` yet (no `lux-render`/`lux-ui` wiring), and nothing is wired
into the `lux` binary. What exists and is verified: `FontSettings`
(family, size, line height, ligatures — the four properties this phase's
deliverable text names), resolved by reading four dotted keys
(`editor.fontFamily`, `editor.fontSize`, `editor.lineHeight`,
`editor.fontLigatures` — the same keys `import::sublime`/`import::vscode`
already produce, so an importer's output needs no translation to be read
back here) straight off a `layering::LayeredConfig`'s already-merged
settings. "Overrides por proyecto y por carpeta" specifically is not a
second override mechanism built for fonts — it's the `Layer::Project`/
`Layer::Folder` precedence the `layering` loop already implemented and
tested, demonstrated here for typography specifically (a folder override
winning over a project override for the same property while leaving
sibling properties still inherited, and reverting to the project value
once the folder override is cleared). Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-config font_settings::` (7
tests). Not built: font settings actually reaching the renderer or being
editable through any UI; `editor.fontLigatures`-driven ligature rendering
itself (`lux-render`'s glyph pipeline, a Phase 2 concern, is unaffected by
this deliverable).

**Scope note (`lux-config::schema` + `lux-config::completion` — Phase 5's
sixth and final deliverable, "Panel de preferencias, o edición de fichero
con autocompletado del esquema (elige uno; documenta la elección)",
tracked in
`docs/receipts/phase-05-panel-preferencias-o-autocompletado-esquema.md`)**:
the "Schema-aware settings-file completion" row above doesn't move — same
reasoning as every row above it: nothing surfaces a completion popup in
any editor UI yet, and nothing is wired into the `lux` binary. **The
choice, documented in `completion`'s own module docs**: file-editing
completion over a preferences UI panel, because every other Phase 5
deliverable in this crate is headless UI-free logic, and a real panel
would need `lux-ui`/`lux` widget/rendering infrastructure that doesn't
exist yet for this purpose — building it inside `lux-config` would pull a
domain crate toward a graphics dependency it has never needed, against
golden rule #2's spirit even though the rule's literal CI gate only checks
`lux-core`. What exists and is verified: `schema::PROPERTIES` (the ten
settings keys `font_settings` and the three `import` modules already use,
written down as one list for the first time, with a value shape —
`String`/`Integer`/`Number`/`Boolean`/`Enum`) and `completion::complete`
(a small hand-written, JSONC-comment-aware state machine — not the full
`jsonc-parser` AST, deliberately: completion has to work on a document
that's mid-edit and very often syntactically incomplete exactly at the
cursor, which a real parser built for complete documents can't do at all;
scoped to the flat, single-level objects every settings document in this
crate actually is). Verified on macOS (aarch64-apple-darwin): `cargo test
-p lux-config` (100 unit tests, including 16 in `completion::tests` and 3
in `schema::tests`). Not built at the time: any UI rendering a completion
popup; hover/documentation-on-demand for a property; validation of a
complete document against the schema — closed in the loop below.

**Scope note (`lux-config::layering::validate` + `schema::to_json_schema`
— closing Phase 5 deliverable 1's last piece, "esquema publicado y
validación con errores útiles señalando línea y columna," tracked in
`docs/receipts/phase-05-esquema-publicado-validacion-linea-columna.md`)**:
the "JSONC config with layered precedence" row (deliverable 1) is now
fully covered by receipts, and "Schema-aware settings-file completion"
(deliverable 6) gained a second, independent consumer of `schema::PROPERTIES`.
Neither row's `not-started` mark moves — same reasoning as every row in
this section: nothing calls `layering::validate` from a real editing
surface yet, and nothing is wired into the `lux` binary. What exists and
is verified: `schema::to_json_schema` (the ten properties exported as a
real draft-07 JSON Schema document — the standard format external tooling
already knows how to consume, not a Lux-only shape) and
`layering::validate` (parses a document with `jsonc_parser::parse_to_ast`
— not `completion`'s own tolerant scanner, since validation is only
meaningful once a document is syntactically complete — and reports every
unknown key and type/enum mismatch with a 1-based line and `char`-counted
column, the same convention `jsonc_parser`'s own parse errors already use;
a syntactically invalid document produces exactly one diagnostic, that
parse error itself, rather than every schema rule silently not running).
Verified on macOS (aarch64-apple-darwin): `cargo test -p lux-config
layering::validate::` (12 tests) plus 3 more in `schema::tests` covering
the JSON Schema export. With this loop, all six of Phase 5's deliverables
have a `Result: passed` receipt.

**Phase 5 close-out.** All six deliverables above have individual receipts
under `docs/receipts/phase-05-*.md` (deliverable 1 across two receipts —
`layering`'s JSONC/precedence/hot-reload half, then `layering::validate`'s
schema/validation half once `schema` existed to validate against — the
other five each a single receipt), each `Result: passed` against its own
gate, and the full workspace gate (`cargo test --workspace`, `cargo
clippy --workspace -- -D warnings`, `cargo deny check`,
`scripts/check-limits.sh`, `scripts/check-core-isolation.sh`) passes clean
at 718 tests, 0 failures. Two things worth stating plainly rather than
leaving implicit:

1. **No capability row above moved to `implemented`, same as Phase 4.**
   Every one of Phase 5's pieces is real, tested, and correct at the
   `lux-config` crate level — but none of it is wired into `crates/lux`
   (the running desktop binary), which today still has no settings file,
   no keymap dispatch, and no preferences surface at all. This is the
   identical, deliberate choice made consistently across all seven loops
   this phase (see each deliverable's own scope note above): Phase 5
   built the complete configuration/personalization *engine*; none of it
   is yet a capability an actual user of the running app has.
2. **All three acceptance criteria are backed by real, automated tests —
   a stronger position than Phase 4's one-of-four.** "Los tres presets de
   keymap pasan un test que verifica los atajos documentados" is
   measured literally: `cargo test -p lux-config keymap:: --test
   keymap_presets`, 7 tests checking real, vendor-sourced shortcuts for
   all three presets. "Una configuración mal formada nunca deja el
   editor inutilizable ni pierde ajustes" and "cambiar la configuración
   se refleja sin reiniciar" are both genuinely exercised by real tests
   against real files — `layering::tests::malformed_layer_keeps_the_previous_good_value_and_reports_the_error`
   and `layering::watch::tests::editing_a_watched_file_hot_reloads_without_restart`,
   among others — not merely argued from the design. The honest caveat:
   "el editor" in both criteria' wording presumes a running editor
   session consuming live config, which doesn't exist yet (point 1
   above) — so what's actually measured is that `LayeredConfig` and
   `ConfigWatcher` themselves have this property at the library level,
   the furthest either criterion can be verified without an editor to
   plug them into. That's a real difference from Phase 4's "not
   measurable with the code as it stands today" gaps: here, the
   mechanism the criteria describe is built, tested, and verified — only
   the last mile (a real window, a real open document, a real keystroke)
   is missing, and that last mile is explicitly out of every Phase 5
   deliverable's scope, not an oversight.

## Git integration (Phase 6, `v0.5.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Gutter diff indicators | not-started | not-started | not-started | not-started | not-started |
| Status summary (branch/dirty/count) | not-started | not-started | not-started | not-started | not-started |
| Diff panel (side-by-side/inline) | not-started | not-started | not-started | not-started | not-started |
| Stage/unstage/commit/amend | not-started | not-started | not-started | not-started | not-started |
| History & blame | not-started | not-started | not-started | not-started | not-started |
| Branches & stash | not-started | not-started | not-started | not-started | not-started |
| Merge conflict resolution | not-started | not-started | not-started | not-started | not-started |

**Scope note (`lux-git::gutter` — diff computation and hunk navigation,
tracked in
`docs/receipts/phase-06-indicadores-cambio-gutter-navegacion-hunks.md`)**:
the "Gutter diff indicators" row above doesn't move — same reasoning as
every `not-started` row elsewhere in this document with a scope note: this
is a headless diff library with nothing wired into `crates/lux-ui`'s
gutter rendering or the running `lux` binary, so no user of the app can
see a change indicator yet. What exists and is verified: `diff_hunks`
(line-level diff between two texts via `gix`'s own `imara-diff`-backed
blob-diff, with Git's histogram algorithm and slider heuristics — the
same engine `gix` itself uses, not a hand-rolled one), `gutter_markers`
(expanding hunks into one per-line marker, Added/Removed/Modified, with a
deletion's marker anchored on the line immediately following it — the
same convention most editors use since a pure deletion owns no line of
its own), `HunkNavigator` (next/previous-hunk lookup by line, wrapping at
either end), and `hunks_against_head` (reads a path's blob at `HEAD` from
a real `gix::Repository` — via `head_tree_id_or_empty`, so an unborn HEAD
diffs against nothing rather than erroring — and diffs it against
caller-supplied working-copy text; a path absent from `HEAD` diffs
against an empty string, so a new file comes back as one `Added` hunk).
No shell-out to `git`, per ADR 0010. Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-git gutter::` (13 tests,
including three against real temporary repositories built with `gix`'s
own object-writing API — no `git` binary involved in the fixtures
either). Not yet built: everything else Phase 6 asks for (branch/status
bar, staged-changes tree, diff panel, staging, commit, history, blame,
branches, stash, merge conflict resolution) and the gutter-rendering
wiring in `lux-ui` itself.

**Scope note (`lux-git::diff_panel` — changes tree and side-by-side diff,
tracked in `docs/receipts/phase-06-panel-cambios-arbol-diff-lado.md`)**:
the "Diff panel (side-by-side/inline)" row above doesn't move — same
reasoning as the row above it: this is a headless view-model library with
nothing wired into `lux-ui`, so no user of the app can see a changes panel
or a diff view yet. Also unmoved because it's genuinely out of scope for
this row: the *inline* half of "side-by-side/inline" isn't built — this
loop's own deliverable title narrows it to "panel de cambios en árbol +
diff lado a lado," inline diff rendering is a separate, later piece. What
exists and is verified: `changed_files` (compares a real `gix::Repository`'s
`HEAD` tree, read via `Tree::traverse().breadthfirst`, against its on-disk
working directory walked with the same gitignore-aware `ignore` crate
`lux-search`/`lux-workspace` already use, classifying every differing path
Added/Modified/Deleted — deleted files are paths present at `HEAD` but
missing from the walk, added files are the reverse, modified files are
byte-for-byte content comparison against the `HEAD` blob), `group_by_folder`
(turns that flat list into a folder tree, sorted, the shape a changes panel
renders), and `side_by_side` (pairs up every line of two texts using
`gutter::diff_hunks` — reusing that module's diff engine rather than a
second one — with blank-padded rows on whichever side has fewer lines
within a hunk, the same alignment a split diff view needs). No shell-out
to `git`, per ADR 0010; the `ignore` crate is a gitignore matcher, not a
`git` process. Verified on macOS (aarch64-apple-darwin): `cargo test -p
lux-git diff_panel::` (10 tests, including three against real temporary
repositories: added/modified/deleted detection, a clean tree reporting no
changes, and confirming a `.gitignore`-matched directory never surfaces as
"added"). Not yet built: staging (this only reads, matching the
deliverable's own "panel de cambios," not "stage/unstage" which is a
separate Phase 6 deliverable), inline diff rendering, renames/copies
detection, and — same as every row in this section — the `lux-ui` wiring.

**Scope note (`lux-git::staging` + `lux-git::commit` — index writes and
commit/amend, tracked in
`docs/receipts/phase-06-stage-unstage-commit-amend.md`)**: the
"Stage/unstage/commit/amend" row above doesn't move — same reasoning as
every scope-noted row in this document: these are real, tested write
operations on a repository's `.git/index` and object database, but
nothing calls them from `lux-ui` or the running `lux` binary yet, so no
user of the app can stage or commit anything through it. Per-hunk staging
(the master prompt's fuller "stage y unstage por fichero y por hunk") is
also not built — this loop's own goal title is "Stage/unstage/commit/
amend" without "por hunk," and partial-hunk staging (constructing a blob
from a subset of a file's hunks) is real additional complexity, not
something to fold in silently. What exists and is verified:
`staging::stage`/`unstage`/`is_staged` (whole-file staging: reads a
file's current on-disk content, writes it as a blob, and records it in
the index at stage 0 via `gix_index`'s own entry-mutation API — including
the executable bit, read from real file metadata, not assumed — replacing
any prior entry for the same path; unstage removes the entry without
touching the working copy) and `commit::commit`/`amend` (builds the new
tree with `gix`'s `Tree::edit()` API, starting from `HEAD`'s tree and
upserting every staged index entry — so files nobody re-staged keep
whatever they were at `HEAD` — then writes a real commit object and moves
`HEAD` to it; `amend` preserves the original commit's author and parents,
same as real `git commit --amend`'s default, only updating the committer
identity/time and, optionally, the message). No shell-out to `git`, per
ADR 0010. One real design finding from this loop, worth stating plainly:
`Repository::commit_as()` — the convenience `commit()` uses — assumes
it's creating an ordinary successor commit, where `HEAD` currently equals
the new commit's first parent, and refuses the ref update otherwise;
`amend` structurally violates that (the new commit's parents are its
*predecessor's* parents, not its predecessor), so `amend` writes the
commit object directly and performs the `HEAD` ref update by hand,
expecting the commit being replaced as the ref's current value instead —
this isn't a workaround for a `gix` limitation, it's the correct lower-level
API for what amend actually does. Verified on macOS (aarch64-apple-darwin):
`cargo test -p lux-git staging::` (5 tests) and `cargo test -p lux-git
commit::` (6 tests), all against real temporary repositories built with
`gix`'s own object-writing API. Not yet built: per-hunk staging, conflict
handling on stage (out of scope until merge conflict resolution, a later
Phase 6 deliverable), and — same as every row in this section — the
`lux-ui` wiring.

**Scope note (`lux-git::history` + `lux-git::blame` — repository/file
history and inline/per-commit blame, tracked in
`docs/receipts/phase-06-historial-blame-linea-commit.md`)**: the "History
& blame" row above doesn't move — same reasoning as every scope-noted row
in this document: real, tested read operations, nothing wired into
`lux-ui` or the running `lux` binary yet, so no user of the app can open a
history or blame view through it. What exists and is verified:
`history::repository_history` (every commit reachable from a starting
point, newest first, via `gix`'s own revision walk —
`Sorting::ByCommitTime(CommitTimeOrder::NewestFirst)` — not a hand-rolled
parent-chain follow), `history::file_history` (the commits that actually
changed a given path's content: for each walked commit, its blob at that
path is compared against every parent's blob at the same path, and it's
included only if it differs from all of them — mirroring `git log`'s
default behavior of hiding a merge that's identical to at least one
parent, without reimplementing full history simplification), and
`blame::blame_by_commit`/`expand_to_lines` (per-commit and per-line blame
views of a file at a given revision). The blame half doesn't reimplement
line attribution at all: `gix` ships a first-party, feature-gated
(`blame`) blame algorithm — `Repository::blame_file()`, backed by the
`gix-blame` crate — that already returns hunks grouped by the commit that
introduced them (`gix_blame::BlameEntry`), which **is** the "por commit"
view directly; `expand_to_lines` is a pure, repository-free transform
that flattens those hunks into one entry per line for the "en línea"
view, so the actual blame computation — the expensive part, a real
history walk with diffing at each step — runs exactly once per call. No
shell-out to `git`, per ADR 0010; per that same ADR, `gix`'s blame
maturity was confirmed by using it directly rather than assumed. Verified
on macOS (aarch64-apple-darwin): `cargo test -p lux-git history::` (4
tests: newest-first ordering with correct parent links, author/message
decoding including a multi-line message's summary line, path-limited
history excluding unrelated commits, an empty result for a path that
never existed) and `cargo test -p lux-git blame::` (4 tests: a
single-commit file blames entirely to that commit, added lines attribute
to the commit that added them while untouched lines keep their original
commit, an edited line attributes to the edit while its neighbors don't,
blaming an earlier revision ignores later history entirely) — all against
real temporary repositories, no `git` binary anywhere in the fixtures.
Not yet built: line-range-limited blame (`gix_blame`'s `BlameRanges`
supports it; this loop always blames the whole file, since nothing in
this deliverable's scope needs partial-file blame yet), and — same as
every row in this section — the `lux-ui` wiring.

**Scope note (`lux-git::branches` + `lux-git::stash`, tracked in
`docs/receipts/phase-06-ramas-stash.md`)**: the "Branches & stash" row
above doesn't move — same reasoning as every scope-noted row in this
document: real, tested write operations, nothing wired into `lux-ui` or
the running `lux` binary yet. What exists and is verified:
`branches::list`/`create` (enumerate `refs/heads/*`; create a new branch
ref, refusing to move an existing one to a different commit — see the
`PreviousValue::MustNotExist` finding below) and `branches::switch` — a
**real, forced checkout**, not a `HEAD`-pointer-only switch: it
materializes the target branch's tree onto the working directory and
index via a new shared primitive, `checkout_tree` (delete paths the old
tree had that the new one doesn't, write every blob the new tree
specifies including its executable bit, rebuild the index to match) — the
same primitive `stash::save`/`pop` reuse to reset/restore the working
copy, so there's one "make the working copy match this tree" code path,
not two. `switch` does not check for or preserve uncommitted changes
first, unlike plain `git switch`; that safety check is left as a
caller/UI policy decision. `stash::save`/`list`/`pop` are a real,
functional stash — snapshot tracked files' current on-disk content into a
commit, reset the working copy to `HEAD`, and bring it back later — using
the standard `refs/stash` ref name, but the stack itself threads through
each stash commit's own second parent rather than `git`'s reflog-based
history, and the stash commit only has one tree (not `git`'s separate
index/worktree distinction); a repository stashed by `lux-git` isn't
guaranteed to also work with the real `git stash` CLI against the same
checkout — see the module's own doc comment for the full compatibility
note. No shell-out to `git`, per ADR 0010; `gix_worktree_state`'s
full checkout API (the `worktree-mutation` feature) was evaluated and
deliberately not used — its `Options` require filesystem capability
probing, `gix-worktree`'s attributes stack, and a real `gix_filter`
pipeline, machinery built for clone-time/`git checkout --force` parity
with `.gitattributes`-driven text conversion, which is more than this
deliverable needs; `checkout_tree` is a focused materializer built from
primitives already used elsewhere in this crate (tree traversal from
`diff_panel`, index writing from `staging`).

**One real `gix` semantics finding, worth stating plainly:**
`PreviousValue::MustNotExist` on a ref *update* (not deletion) only
rejects the write if the existing ref's target actually differs from the
new value — re-running `repo.reference(name, same_target, MustNotExist,
...)` is treated as an idempotent no-op success, not an error, per
`gix-ref`'s own file-transaction source
(`store/file/transaction/prepare.rs`: `(MustNotExist, Some(existing)) =>
if existing.target != *new { Err(...) }`). `branches::create`'s test
suite originally asserted create-twice-with-the-same-target fails, and
that assertion was simply wrong about `gix`'s semantics, caught
immediately by the test actually running (not by reading docs first);
fixed by testing the real failure mode (creating an existing branch name
pointing at a *different* commit) and adding a second test that pins down
the idempotent-same-target case as intentional, documented behavior
rather than leaving it as a surprise. Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-git branches::` (6 tests) and
`cargo test -p lux-git stash::` (5 tests), all against real temporary
repositories built with `gix`'s own object-writing API. Not yet built:
per-hunk or partial staging feeding into stash (stash always snapshots
every tracked file's current on-disk content, matching plain `git
stash`'s default — no `-u`/untracked-file support), conflict handling
when a checkout would overwrite unsaved changes, and — same as every row
in this section — the `lux-ui` wiring.

**A second real `gix` finding, caught by CI rather than local testing**:
`branches::create`/`switch`, `commit::amend`, and `stash::save`/`pop` all
write a ref update with a ref-log entry, and every one of them originally
relied on `gix`'s ambient `Repository::committer()` to supply that
ref-log's identity (via the `reference()`/`edit_reference()`
convenience methods). Per `gix`'s own documented design
(`repository/identity.rs`: "There is no notion of a default user like in
git, and instead failing to provide a user is fatal"), that call returns
`None` — a hard `MissingCommitter` error — unless `user.name`/`email` is
configured somewhere `gix` can see it, with **no** fallback to a guessed
`user@host` the way real `git` has for non-commit ref updates. Every
local run in this phase passed because the development machine's global
`~/.gitconfig` has an identity configured; the `macOS` GitHub Actions
runner has none, so every test exercising these four operations failed
there with `RefEdit(FileTransactionCommit(CreateOrUpdateRefLog(MissingCommitter)))`
the first time this branch's PR ran CI — a genuine portability bug, not
a CI quirk, since the same failure would hit any real user who has git
installed but hasn't set an identity yet. Fixed by threading explicit
`committer_name`/`committer_email` parameters through all four functions
(matching the pattern `commit`/`amend` already used) and calling `gix`'s
`edit_references_as(edits, Some(committer))` instead of the ambient-only
convenience methods. Verified both normally and with a from-scratch,
git-identity-free environment (`env -i HOME=<empty> GIT_CONFIG_NOSYSTEM=1
GIT_CONFIG_GLOBAL=/dev/null cargo test -p lux-git`): all 74 tests pass
either way, confirming the fix removes the dependency rather than just
happening to work on this machine.

**Scope note (`lux-git::merge_conflicts`, tracked in
`docs/receipts/phase-06-resolucion-conflictos-merge.md`)**: the "Merge
conflict resolution" row above doesn't move — same reasoning as every
scope-noted row in this document: a real, tested capability, nothing
wired into `lux-ui`'s editor (no "Accept Current / Incoming / Both"
commands exist yet — this loop builds the primitives such an action would
call, not the action itself). What exists and is verified: `merge()` — a
genuine three-way merge computed by `gix`'s own merge engine
(`Repository::merge_commits()`, automatic merge-base detection), which
materializes its result onto the working directory and index via the
same `checkout_tree` primitive branch-switching and stash use, so a
merge with conflicts leaves real `<<<<<<<`/`=======`/`>>>>>>>` markers in
the working tree files, exactly as real `git merge` does; `resolve()` —
given a conflict and an `Ours`/`Theirs`/`Both` choice, writes the chosen
content to the working file and stages it, the exact primitive an
"Accept Current / Incoming / Both" editor action would call per conflict;
and `complete()` — writes the two-parent merge commit from whatever's
currently staged once conflicts are resolved. No shell-out to `git`, per
ADR 0010 — and per that same ADR, `gix-merge`'s maturity was confirmed by
using it directly (same discipline that already paid off for blame
earlier in this phase) rather than assumed or worked around. **Scope is
deliberately narrower than everything `gix-merge` can report**: only
*content conflicts* (both sides modified, or both sides independently
added, the same path) are turned into a `MergeConflict` — rename
conflicts and delete/modify conflicts are real, detected outputs this
loop doesn't yet surface as resolvable; `merge()` reports those
explicitly via `MergeConflictError::UnsupportedConflictShape` rather than
silently dropping or misrepresenting them, so a caller always knows when
it hit a shape this module doesn't handle instead of getting a
subtly-wrong result. Verified on macOS (aarch64-apple-darwin): `cargo
test -p lux-git merge_conflicts::` — 7 tests, 0 failures, 0 ignored,
against a real temporary repository with an **induced** conflict (two
branches independently edit the same line of the same file from a shared
base) — not a synthetic/mocked conflict object: a merge with no real
conflicts leaves the non-conflicting change already merged on disk, the
induced conflict is detected with correct `ours`/`theirs` blob content
and real conflict markers written to the working copy, each of
`Ours`/`Theirs`/`Both` resolves correctly, and completing the merge
writes a real two-parent commit whose tree reflects the resolution
chosen. Not yet built: rename and delete/modify conflict resolution,
merge abort, and — same as every row in this section — the `lux-ui`
wiring (the "Accept Current / Incoming / Both" editor commands
themselves).

**Closing loop: `lux-git::status`, inline diff, per-hunk staging, and
verification against a real large repository** (tracked in
`docs/receipts/phase-06-completar-pendientes.md`). A `/close-phase 6`
audit found four real gaps against the master prompt's full entregable
list and Phase 6's own acceptance criteria, none covered by the six
receipts above: no repository-status summary existed at all; "diff lado
a lado **y en línea**" only had the side-by-side half; "stage... **y por
hunk**" only staged whole files; and — most importantly — nothing in
Phase 6 had actually been run against a real, large repository, which
the phase's acceptance criteria explicitly requires ("usa el propio
repo de Rust o de Linux como caso de prueba"). This loop closed the
first three and ran the fourth, which surfaced two real bugs neither
the small-repo test suite nor manual reasoning had caught:

- **`status`/`diff_panel::changed_files` bug**: a tracked file that also
  matches a `.gitignore` rule added *after* it was committed (a common
  real pattern — rust-lang/rust has 12 such files, e.g. `.vscode/
  settings.json` committed early, ignored later) was reported as
  **deleted**, because the gitignore-aware directory walk this crate
  uses to discover files on disk correctly skips it, and the old code
  treated "not found by the walk" as "gone." Real `git status` never
  lets `.gitignore` hide an already-tracked path — the rule only ever
  governs *un*tracked-file discovery. Fixed: every tracked path is now
  checked directly against disk, independent of the gitignore-aware
  walk, which is now used only to discover genuinely untracked files.
  Regression test: `diff_panel::tests::against_real_repo::
  a_tracked_file_that_matches_a_later_gitignore_rule_is_not_reported_as_deleted`.
- **`stash::save` bug**: a file that was `git add`-ed (staged) but never
  committed was invisible to `save` — it only scanned `HEAD`'s tree, not
  the index — so stashing right after staging a new file failed with
  "nothing to stash" despite a real staged change sitting there. Real
  `git stash` (even without `-u`) does capture staged-but-uncommitted
  files; only genuinely untracked-and-unstaged files are excluded by
  default. Fixed: the snapshot now covers the union of `HEAD`'s tree and
  the current index, not `HEAD`'s tree alone. Regression test:
  `stash::tests::save_includes_a_newly_staged_file_not_yet_committed`.

Both were caught specifically *because* the verification ran against a
repository with real, messy history (rust-lang/rust, shallow-cloned:
~62,000 files, a 71,164-line real source file) rather than only the
small, clean fixtures every other Phase 6 test builds by hand — neither
bug is reachable from a repo with a handful of files and no
pre-existing `.gitignore`/staging history. This is exactly the value
the phase's large-repo acceptance criterion is protecting against, and
it paid off on the first real run.

**One gap found and deliberately not fixed, per ADR 0010's "write up the
finding, don't shell out" instruction**: `blame::blame_by_commit` fails
on shallow clones (`git clone --depth N`) once its history walk needs to
cross the shallow boundary — `gix-blame` 0.16.0 reads the raw `parent`
field from the boundary commit's object bytes and tries to fetch it,
rather than consulting `.git/shallow` the way `git blame` itself does,
and fails with `NotFound` on an object that was never fetched by design.
Confirmed directly: `git cat-file -t <that oid>` also fails locally (the
object genuinely isn't present — this is `git`'s intended shallow-clone
behavior, not corruption), while the real `git blame` binary succeeds
because it treats the boundary commit as parentless. This is narrow in
practice — a normal (non-shallow) clone, the ordinary case for a
repository opened in an editor, has every object `blame` might need, so
this shouldn't affect real usage — but it's a genuine, reproducible
`gix` limitation worth tracking if `lux` ever needs to support opening
shallow clones directly (CI checkouts, some `git worktree` setups).

What else exists and is verified from this loop: `status::status`
(current branch — named, detached, or unborn — plus a dirty/clean flag
and changed-file count, reusing `diff_panel::changed_files` for the
count rather than a second change-detection path); `diff_view::
inline_diff` (a unified-diff view alongside the existing `side_by_side`,
both now living in their own module split out of `diff_panel` once
adding it pushed that file over the 600-line cap — pure rendering,
`diff_panel` keeps just the changes-tree data source); and `staging::
unstaged_hunks`/`stage_hunk` (per-hunk staging: diffs a path's currently
staged-or-`HEAD` content against its working-copy content via `gutter::
diff_hunks`, and stages one selected hunk by applying it onto that same
base — matching `git add -p`, verified with two independent edits to one
file staged one hunk at a time, each producing exactly the expected
partial content). Verified on macOS (aarch64-apple-darwin): `cargo test
-p lux-git` — 74 tests, 0 failures, 0 ignored, plus the full workspace
gate (`cargo test --workspace`, `cargo clippy --workspace -- -D
warnings`, `cargo deny check`, `scripts/check-limits.sh`,
`scripts/check-core-isolation.sh`) all clean — and, separately, the
manual large-repository verification pass described above (not part of
the automated suite — a 459 MB shallow clone isn't something to bundle
into routine `cargo test`, matching how every other loop's "verified
on..." line already describes a manual pass, not a promise that the
exact commands are checked into CI). Still not built, and not attempted
here because it is out of `lux-git`'s scope by golden rule 2, not an
oversight: the "Accept Current / Incoming / Both" commands themselves
(they live in the editor, which is `lux-ui`, and `lux-ui` doesn't
consume `lux-git` at all yet) and rename/delete-modify conflict
resolution (unchanged from the prior loop's note).

## LSP client (Phase 7, `v0.6.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Server lifecycle management | not-started | not-started | not-started | excluded (no child processes on iOS — ADR 0006) | not-started |
| Incremental document sync | not-started | not-started | not-started | excluded | not-started |
| Diagnostics panel | not-started | not-started | not-started | excluded | not-started |
| Autocomplete | not-started | not-started | not-started | excluded | not-started |
| Navigation (definition/references/symbols) | not-started | not-started | not-started | excluded | not-started |
| Hover / signature help / inlay hints | not-started | not-started | not-started | excluded | not-started |
| Code actions + `WorkspaceEdit` | not-started | not-started | not-started | excluded | not-started |
| Cross-file rename | not-started | not-started | not-started | excluded | not-started |
| Formatting | not-started | not-started | not-started | excluded | not-started |

## Terminal (Phase 8, `v0.7.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Real PTY terminal | not-started | not-started | not-started | excluded (no child processes on iOS — ADR 0006) | not-started |
| ConPTY | n/a | not-started | n/a | n/a | n/a |
| Scrollback, search, selection | not-started | not-started | not-started | excluded | not-started |
| Multiple named terminals / shell profiles | not-started | not-started | not-started | excluded | not-started |
| Build-error link detection | not-started | not-started | not-started | excluded | not-started |
| Task panel with cancellation | not-started | not-started | not-started | excluded | not-started |

## Plugins (Phase 9, `v0.8.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| `wasmtime` plugin host | not-started | not-started | not-started | not-started (subject to App Review — ADR 0006) | not-started |
| Capability-scoped permissions | not-started | not-started | not-started | not-started | not-started |
| Sandboxed CPU/memory limits | not-started | not-started | not-started | not-started | not-started |
| Extension points (commands, grammars, themes...) | not-started | not-started | not-started | not-started | not-started |
| Local plugin install | not-started | not-started | not-started | not-started | not-started |

## Accessibility, i18n & polish (Phase 10, `v0.9.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Full keyboard navigation | not-started | not-started | not-started | not-started | not-started |
| Screen reader support | not-started (VoiceOver) | not-started (NVDA) | not-started (Orca) | not-started | not-started (TalkBack) |
| High contrast / UI scaling / reduced motion | not-started | not-started | not-started | not-started | not-started |
| i18n (es/en) | not-started | not-started | not-started | not-started | not-started |
| RTL layout | not-started | not-started | not-started | not-started | not-started |

## Packaging & distribution (Phase 11, `v1.0.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Signed, notarized installer | not-started (.dmg) | not-started (.msi + winget) | not-started (.deb/.rpm/AppImage/Flatpak) | n/a (App Store, Phase 12) | n/a (Play Store, Phase 13) |
| Auto-update | not-started | not-started | not-started | n/a | n/a |
| Local crash reports (no auto-send) | not-started | not-started | not-started | not-started | not-started |

## Mobile shells (Phase 12 `v1.1.0`, Phase 13 `v1.2.0`)

| Capability | iPadOS/iOS | Android |
|---|---|---|
| Native platform-trait backend (UIKit / Kotlin+JNI) | not-started | not-started |
| Mobile text input (IME, selection handles, system menu) | not-started | not-started |
| External keyboard / trackpad / Stage Manager (iPad) | not-started | n/a |
| Sandboxed file access (`UIDocumentPicker` / SAF) | not-started | not-started |
| `accesskit_ios` / `accesskit_android` | not-started | not-started |
| App Store / Play Store signed build | not-started | not-started |

## Experimental

_Nothing is behind an `experimental.*` flag yet._

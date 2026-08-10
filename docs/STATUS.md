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

**Known gap**: the "open a 100 MB file" performance budget's RSS half (< 3×
file size) measures 3.16×, over budget — see `docs/PERFORMANCE.md`'s
measurement history for the root cause (three simultaneous copies of the
file's bytes during `Buffer::open`) and why it wasn't fixed in the same pass
that found it.

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

## Configuration & personalization (Phase 5, `v0.4.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| JSONC config with layered precedence | not-started | not-started | not-started | not-started | not-started |
| Keymap presets (Sublime/VS Code/JetBrains) | not-started | not-started | not-started | not-started | not-started |
| `.editorconfig` / `.gitattributes` | not-started | not-started | not-started | not-started | not-started |
| Settings importers | not-started | not-started | not-started | not-started | not-started |
| Font settings + overrides | not-started | not-started | not-started | not-started | not-started |

## Git integration (Phase 6, `v0.5.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Gutter diff indicators | not-started | not-started | not-started | not-started | not-started |
| Diff panel (side-by-side/inline) | not-started | not-started | not-started | not-started | not-started |
| Stage/unstage/commit/amend | not-started | not-started | not-started | not-started | not-started |
| History & blame | not-started | not-started | not-started | not-started | not-started |
| Branches & stash | not-started | not-started | not-started | not-started | not-started |
| Merge conflict resolution | not-started | not-started | not-started | not-started | not-started |

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

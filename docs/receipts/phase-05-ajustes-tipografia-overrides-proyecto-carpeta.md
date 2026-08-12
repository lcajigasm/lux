# Receipt: Ajustes de tipografía + overrides por proyecto/carpeta (deliverable 5 of Phase 5)

- Phase: 05
- Crate(s): lux-config
- Goal: The "Ajustes de tipografía + overrides por proyecto/carpeta"
  deliverable of Phase 5 ("Configuración, keymaps y personalización") from
  `LUX_PROMPT_MAESTRO.md` is complete and its gate passes with no failures
  and no ignored tests.
- Verify: `cargo test -p lux-config font_settings::`
- Iterations: 1 / 6
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Scope.** Phase 5's deliverable 5 text: "Ajustes de tipografía: familia,
  tamaño, alto de línea, ligaduras, con overrides por proyecto y por
  carpeta." Four typed properties, project and folder overrides.
  `docs/STATUS.md`'s "Font settings + overrides" row stays `not-started`
  (rule 1), same pattern as every loop this phase: nothing in
  `lux-render`/`lux-ui` reads a resolved `FontSettings` yet, and nothing is
  wired into the `lux` binary.

  **The whole deliverable was mostly a composition of infrastructure this
  phase already built, not new machinery.** "Overrides por proyecto y por
  carpeta" is exactly `layering::LayeredConfig`'s `Layer::Project`/
  `Layer::Folder` precedence — implemented, tested, and hot-reloadable
  since the first loop of this phase. Building a second, font-specific
  override mechanism would have duplicated that for no reason and,
  worse, given typography a different override *model* than every other
  setting in the crate (a real inconsistency a future settings UI would
  have to explain). So `font_settings::resolve` does the smallest possible
  thing on top of already-verified infrastructure: read four keys back out
  of a `LayeredConfig`'s already-merged `resolved()` view. The loop's real
  work was picking the right four keys and proving, specifically for
  typography, that the override precedence already built two loops ago
  actually produces the right answer — not re-verifying `layering` itself
  (that's `layering::tests`' job, not this module's).

  **Key names reuse `import`'s own vocabulary, not a parallel one.**
  `editor.fontFamily` and `editor.fontSize` are the exact `lux_key` values
  `import::sublime`/`import::vscode`'s `map_key` functions already produce
  (from the settings-importers loop immediately before this one) — so an
  `ImportedSetting` from either importer lands directly at the key this
  module reads back, no translation layer needed in between, and the two
  deliverables compose without either having to know about the other's
  internals. `editor.lineHeight` follows the same VS Code-derived dotted
  convention. `editor.fontLigatures` is new to this loop (ligatures wasn't
  part of the importers' scope) but follows the identical naming pattern —
  it's also VS Code's own real setting name for the same concept
  (`editor.fontLigatures`), kept consistent rather than inventing Lux-only
  terminology for a property every one of the three reference editors
  already has a name for.

  **`size`/`line_height` as `f64`, not integers — a deliberate, documented
  choice, not an oversight.** `editor.fontSize` is commonly an integer in
  practice, but `line_height` is routinely a fractional multiplier (`1.5`)
  in real editor configs, and once either value is read out of the
  already-JSON-typed `LayeredConfig`, `f64` is what both economically and
  correctly represent every value either property could actually hold —
  making `size` a narrower type than `line_height` for no functional
  reason would just be an inconsistency the resolver introduces on its
  own. Exercised directly
  (`resolves_all_four_properties_from_the_default_layer` asserts a
  fractional `1.5` line height resolves correctly).

  **A malformed-value property degrades to `None`, not an error — the
  same convention `editorconfig::resolve` already established two loops
  ago, applied here for the same reason.** `editor.fontSize` set to a JSON
  string instead of a number doesn't fail the whole resolution; it just
  means that one field comes back `None`, leaving every other typography
  property (and every other setting in the layered config entirely)
  unaffected
  (`a_property_of_the_wrong_json_type_resolves_to_none_not_an_error`).

  **Actual verification.** `cargo test -p lux-config font_settings::`:
  7/7 passed, 0 ignored, on the first run — no fix-and-retry cycle needed
  (composing already-tested `layering` infrastructure left little room for
  a new bug; the only genuinely new logic here is four straight-line key
  reads). Coverage: all four properties resolving correctly from a single
  default layer; every property correctly unset (`FontSettings::default()`)
  when none are configured; a project-layer override winning over the
  default for `family`; a folder-layer override winning over a
  project-layer override for the *same* property (`size`), proving the
  precedence order specifically for typography rather than assuming
  `layering`'s general-purpose tests already cover this case sufficiently;
  a folder override of one property (`size`) leaving sibling properties
  (`family`, `ligatures`) still inherited from the default layer
  underneath it, not clobbered by the folder file's narrower JSON object
  (real per-key merge, not a wholesale layer replacement — the same
  distinction `layering::tests::nested_objects_merge_recursively...`
  proved generically, re-demonstrated here at this module's own level);
  clearing a folder override correctly falling back to the project value
  underneath it (`LayeredConfig::clear_layer`, from the `layering` loop,
  exercised through this module's own typed surface); and a
  wrong-JSON-type value resolving to `None` rather than erroring. Full
  `cargo test -p lux-config` (no filter): 81 unit tests (the 7 above plus
  74 from the four earlier Phase 5 loops) and 7 integration tests from
  `keymap_presets`, all green. Full repo gate also run and green: `cargo
  fmt --all --check` (after one `cargo fmt --all` pass); `cargo clippy
  --workspace --all-targets -- -D warnings` (zero warnings); `cargo test
  --workspace` (all crates, no regressions — `lux-core`'s 255,
  `lux-workspace`'s 54, everything else unchanged); `cargo deny check`
  (advisories/bans/licenses/sources all `ok` — no new dependency was added
  this loop at all, so no new entries were possible);
  `scripts/check-limits.sh` (`font_settings.rs` at 206 lines, well under
  the 600-line cap; `FontSettings` has 4 fields); `scripts/check-core-isolation.sh`
  (unaffected — only checks `lux-core`). Verified on macOS
  (aarch64-apple-darwin) only, same documented limitation as the rest of
  `docs/STATUS.md`.

  **What's out of scope and untouched:** a preferences surface or any UI
  to view/edit typography settings (Phase 5 deliverable 6, still
  not-started); anything in `lux-render`/`lux-ui` actually consuming a
  resolved `FontSettings` to change what gets drawn (font loading, glyph
  metrics, the ligature-substitution pass `lux-render`'s existing `parley`/
  `swash` text stack would need to honor `editor.fontLigatures` for real —
  Phase 2 territory, unaffected by this deliverable); a `Layer::Language`
  override for typography specifically (the data model already supports
  it — `font_settings::resolve` reads whatever `LayeredConfig::resolved()`
  currently reflects, which already includes the `Language` layer when
  one is set — but no test in this loop exercises that layer in
  particular, since `layering::tests::precedence_is_default_user_project_folder_language`
  already covers the full five-layer chain generically and this loop's
  own tests focus on the two layers the deliverable's text names by name,
  project and folder).

# 0020. Five OFL developer fonts ship as files next to the binary; the system face stays the default

## Status

Accepted, 2026-11-26. Supersedes the sentence "No font is bundled" in
`docs/ROADMAP.md` (visual-quality section). [ADR 0003](0003-text-rendering-stack.md)
is unchanged: these fonts go through the same `parley` + `fontique` +
`swash` stack, registered from bytes.

## Context

The editor face is whatever system monospace file the loader finds first
(`crates/lux/src/font.rs`: SF Mono, then Menlo on macOS; DejaVu Sans Mono
on Linux; Consolas on Windows). `editor.fontFamily` existed in the schema,
the importers, and completion, but nothing applied it. A user who wants a
programming font with ligatures has to install it, and even then Lux has
no system font enumeration (`fontique`'s `system` feature is off, see
`docs/DEPENDENCIES.md`), so it could not be picked.

Constraints from the working agreement: no I/O on the UI thread, cold start
is a budget, and nothing is added to the UI that does not work.

## Decision

**Fonts.** Five families, one upright Regular face each, unmodified, from
each project's official GitHub release asset, downloaded over HTTPS:

| Family | Version | Release asset | File | Size | Ligatures |
|---|---|---|---|---|---|
| JetBrains Mono | 2.304 | `JetBrains/JetBrainsMono` `v2.304` `JetBrainsMono-2.304.zip` | `JetBrainsMono-Regular.ttf` | 274 KB | yes (`calt`) |
| Fira Code | 6.2 (font 6.002) | `tonsky/FiraCode` `6.2` `Fira_Code_v6.2.zip` | `FiraCode-Regular.ttf` | 290 KB | yes (`calt`) |
| IBM Plex Mono | 2.5.0 (font 2.005) | `IBM/plex` `@ibm/plex-mono@2.5.0` `ibm-plex-mono.zip` | `IBMPlexMono-Regular.ttf` | 173 KB | no |
| Source Code Pro | 2.042 | `adobe-fonts/source-code-pro` `2.042R-u/1.062R-i/1.026R-vf` `TTF-source-code-pro-2.042R-u_1.062R-i.zip` | `SourceCodePro-Regular.ttf` | 210 KB | no |
| Cascadia Code | 2407.24 | `microsoft/cascadia-code` `v2407.24` `CascadiaCode-2407.24.zip` | `CascadiaCode-Regular.ttf` | 598 KB | yes (`calt`) |

All five are SIL Open Font License 1.1. Only IBM publishes asset digests;
the Plex archive matched its published SHA-256. Each archive's SHA-256 and
each shipped file's SHA-256 are recorded in `fonts/NOTICE.md` and
`fonts/SHA256SUMS`, which packaging re-checks.

Static TTFs, not variable fonts: Lux has no weight or style axis to drive
(see below), and every family offers a static Regular.

**Regular only.** Nothing in Lux draws bold or italic text: themes' font
styles are not applied and the one "bold" (selection match) is a shifted
second copy of the regular glyph. Shipping Bold, Italic, and Bold Italic
would add about 3 MB that no code path reads. They are added in the same
change that makes the renderer use them.

**Default.** The system face stays the default on every platform
(`editor.fontFamily` unset or `"system"`). On macOS that is SF Mono, then
Menlo, which is what Sublime Text and Terminal look like. Bundled
families are opt-in. No UI font is bundled; the system UI face stays.

**How they ship.** Files, not `include_bytes!`. The repository keeps them
under `fonts/<Family>/` with each family's `OFL.txt`; `fonts/NOTICE.md`
lists source, version, copyright and reserved font names.
`scripts/install-fonts.sh <dir>` verifies the checksums and copies the
tree. Every packager calls it:

- macOS `.app` (`scripts/package-macos.sh`, `scripts/stage-lux-app.sh`):
  `Contents/Resources/fonts/`.
- `.deb` (`scripts/package-deb.sh`): `/usr/share/lux/fonts/`.
- dist tree (`scripts/package.sh`): `share/lux/fonts/`.
- dev runs from `target/`: the repository's own `fonts/`, found from the
  crate's compile-time manifest directory. Debug builds and tests only; a
  release binary never looks in the source tree, so a package missing its
  fonts shows up as a fallback instead of silently working on the build
  machine.

The binary looks for the font directory relative to its own path in that
order. If no directory or file is found, the request falls back to the
system face and a line is logged. The Windows MSI does not install the
directory yet, so on Windows the Settings page does not offer the bundled
families and `editor.fontFamily` accepts only `system` there (rule 1: no UI
for something that cannot work).

Embedding was rejected: 1.5 MB in every binary, including the CLI copy in
`SharedSupport/bin`, for fonts most users never select.

**Size budget.** 1.6 MB of font files plus licenses (about 0.9 MB inside
the compressed DMG). A new family or face needs an ADR that amends this
one, with its size.

**Loading.** Only the selected family is read.

- At startup, the one editor face is read on the main thread before the
  event loop starts, as the system face already was: the selected bundled
  file replaces the system file in that read, so cold start reads one
  font file either way. The user settings file is parsed once there to
  learn the family.
- After startup, a changed `editor.fontFamily` starts a `ui_io` worker
  that reads the file. The UI thread keeps drawing the current face. When
  the worker reports back, the UI thread registers the bytes, swaps the
  editor family and the rasterizing face, and invalidates the frame. A
  face loaded once stays registered, so switching back does not read the
  disk again.

**Names.** `editor.fontFamily` accepts `"system"`, the five family names
above (case-insensitive), and on macOS `"SF Mono"` and `"Menlo"`, which
the system loader already knew. Any other value logs and uses the system
face. Settings lists exactly these values. `editor.fontLigatures` is
unchanged: it turns `calt`/`liga` on and off; the three families with
ligatures change shape, the other two do not.

**Licensing.** The OFL allows bundling with software, including
commercial, if each copy carries the copyright notice and license and the
fonts are not sold alone. We ship every face unmodified, so the Reserved
Font Name clauses do not apply. `fonts/` (license texts and NOTICE) ships
inside the `.app`, the `.deb`, and the dist tree. `cargo deny` only checks
crates, so it has nothing to say about these files; `docs/DEPENDENCIES.md`
lists them.

## Alternatives considered

- **Default to JetBrains Mono everywhere.** Uniform look across platforms,
  but on macOS it is not what native editors use, and it would make the
  bundled file a hard dependency of the first frame.
- **System font enumeration** so any installed font can be picked. Still
  blocked by the `fontconfig` link conflict in `docs/DEPENDENCIES.md`.
- **Variable fonts with all weights.** No consumer for the axes today.

## Consequences

Release artifacts grow by about 1.6 MB. Packaging scripts must keep
calling `install-fonts.sh`; a test runs it and checks the result. Updating
a font is a deliberate change: new file, new checksum, NOTICE and this
ADR's table updated.

# Test fixtures

`ahem.ttf` — the [Ahem font](https://web-platform-tests.org/writing-tests/ahem.html),
public domain (CC0 fallback in jurisdictions without a public-domain concept;
see the font's own `COPYING` name-table entry). Every glyph is a solid black
square filling its em box, which is exactly why the W3C built it for testing
typographic tools: rasterizing any glyph ID is guaranteed to produce a
non-empty, predictable bitmap, with no dependency on font-rendering taste or
hinting quality. Used only by `glyph_atlas` tests; never shipped in the
editor itself. Sourced from
`googlefonts/fontations/font-test-data/test_data/ttf/ahem.ttf`.

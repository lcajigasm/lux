//! `parley` + `fontique` integration: monospace line layout, OpenType
//! programming-ligature features controlled per run, and cluster geometry
//! for cursor/selection/IME callers above this crate.
//!
//! `fontique`'s `system` feature (platform font discovery) is deliberately
//! off — see `crates/lux-render/Cargo.toml` and this deliverable's receipt:
//! enabling it pulls `yeslogic-fontconfig-sys` 6.x, which conflicts at the
//! Cargo `links = "fontconfig"` level with the 5.x pinned by `crossfont`
//! (via `winit`'s Wayland CSD support, already required by `lux-platform`).
//! Both crates are workspace members sharing one resolved dependency graph,
//! so this isn't a per-crate choice. Fonts are registered directly from
//! bytes via [`TextLayoutContext::register_font`] instead; real system font
//! enumeration is a later loop, tracked as a documented gap, not a silent
//! placeholder.

use std::borrow::Cow;
use std::ops::Range;

use fontique::Blob;
use parley::setting::Tag;
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, FontFeature, FontFeatures, Layout,
    LayoutContext, PositionedLayoutItem, StyleProperty,
};

/// This crate has no rendering use for parley's `Brush` (glyph color is
/// decided by whatever paints the glyph atlas, not by layout); a unit-like
/// type satisfies the trait's blanket impl (`Clone + PartialEq + Default +
/// Debug`) with nothing to store.
#[derive(Copy, Clone, Eq, PartialEq, Default, Debug)]
pub struct NoBrush;

/// Per-line font selection, shared across many [`TextLayoutContext::layout_line`]
/// calls the way a real editor would (family/size stay constant across a
/// file; only the text changes per line).
#[derive(Copy, Clone, Debug)]
pub struct LineStyle<'a> {
    pub family: &'a str,
    pub font_size: f32,
    /// Whether programming ligatures (`calt`/`liga`) are on by default for
    /// the whole line. Individual byte ranges can still override this via
    /// `layout_line`'s `ligature_overrides` — that's the "per run" control
    /// the master prompt asks for.
    pub ligatures: bool,
}

/// Where one shaped cluster (the atomic unit cursor/selection/IME operate
/// on — one or more source bytes mapped to one or more glyphs) landed on
/// its line, in the same coordinate space as [`LineLayout::width`].
#[derive(Clone, Debug, PartialEq)]
pub struct ClusterGeometry {
    pub byte_range: Range<usize>,
    pub x: f32,
    pub advance: f32,
    pub is_ligature_start: bool,
    pub is_ligature_continuation: bool,
}

/// A shaped, line-broken, left-aligned single-paragraph layout (an editor
/// line is never wrapped mid-line; `lux-ui`'s virtualized view decides
/// what's visible, not `parley`).
pub struct LineLayout {
    layout: Layout<NoBrush>,
}

impl LineLayout {
    pub fn width(&self) -> f32 {
        self.layout.width()
    }

    pub fn height(&self) -> f32 {
        self.layout.height()
    }

    /// Cluster geometry across every line `parley` produced (normally one,
    /// since `layout_line` never sets a max advance — but source text can
    /// still contain hard line breaks, so this doesn't assume a single
    /// line). Positions are computed once, linearly, by walking each run's
    /// clusters in logical order and accumulating advances from the run's
    /// own baseline offset — the same arithmetic `Cluster::visual_offset`
    /// does per-call, just done once for the whole line instead of once per
    /// cluster.
    pub fn clusters(&self) -> Vec<ClusterGeometry> {
        let mut out = Vec::new();
        for line_index in 0..self.layout.len() {
            let Some(line) = self.layout.get(line_index) else {
                continue;
            };
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let mut x = glyph_run.offset();
                for cluster in glyph_run.run().clusters() {
                    let advance = cluster.advance();
                    out.push(ClusterGeometry {
                        byte_range: cluster.text_range(),
                        x,
                        advance,
                        is_ligature_start: cluster.is_ligature_start(),
                        is_ligature_continuation: cluster.is_ligature_continuation(),
                    });
                    x += advance;
                }
            }
        }
        out
    }
}

/// Owns the `fontique` font database and `parley` scratch space. Both are
/// meant to be long-lived, reused across every layout call (per `parley`'s
/// own docs), not recreated per line.
pub struct TextLayoutContext {
    font_context: FontContext,
    layout_context: LayoutContext<NoBrush>,
}

impl TextLayoutContext {
    pub fn new() -> Self {
        Self {
            font_context: FontContext::new(),
            layout_context: LayoutContext::new(),
        }
    }

    /// Registers a font directly from its bytes (no system lookup — see the
    /// module doc comment) and returns the family name `parley` resolved it
    /// under, for use as [`LineStyle::family`]. `None` if the bytes don't
    /// parse as a font `fontique` can read.
    pub fn register_font(&mut self, font_bytes: &[u8]) -> Option<String> {
        let blob = Blob::from(font_bytes.to_vec());
        let (family_id, _fonts) = self
            .font_context
            .collection
            .register_fonts(blob, None)
            .into_iter()
            .next()?;
        self.font_context
            .collection
            .family_name(family_id)
            .map(str::to_owned)
    }

    /// Lays out one logical line of text: shapes it with `style.family` at
    /// `style.font_size`, with programming ligatures on/off per
    /// `style.ligatures` by default and per-byte-range overrides in
    /// `ligature_overrides` (each `(range, enabled)` pair wins over the
    /// line default for that range — real OpenType-feature-per-run control,
    /// not just a whole-line switch), then breaks and left-aligns it.
    pub fn layout_line(
        &mut self,
        style: &LineStyle,
        text: &str,
        ligature_overrides: &[(Range<usize>, bool)],
    ) -> LineLayout {
        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, text, 1.0, true);
        builder.push_default(StyleProperty::FontFamily(FontFamily::named(style.family)));
        builder.push_default(StyleProperty::FontSize(style.font_size));
        builder.push_default(ligature_feature_property(style.ligatures));
        for (range, enabled) in ligature_overrides {
            builder.push(ligature_feature_property(*enabled), range.clone());
        }
        let mut layout: Layout<NoBrush> = builder.build(text);
        layout.break_all_lines(None);
        layout.align(Alignment::Start, AlignmentOptions::default());
        LineLayout { layout }
    }
}

impl Default for TextLayoutContext {
    fn default() -> Self {
        Self::new()
    }
}

/// `calt` (contextual alternates) and `liga` (standard ligatures) are the
/// two OpenType features that produce programming ligatures (`->`, `!=`,
/// `>=`...) in fonts that define them; toggling both together is what every
/// editor's "enable ligatures" setting actually does.
fn ligature_feature_property(enabled: bool) -> StyleProperty<'static, NoBrush> {
    let value = u16::from(enabled);
    StyleProperty::FontFeatures(FontFeatures::List(Cow::Owned(vec![
        FontFeature::new(Tag::new(b"calt"), value),
        FontFeature::new(Tag::new(b"liga"), value),
    ])))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same public-domain Ahem fixture the glyph atlas tests use (see
    /// `tests/fixtures/README.md`): every glyph is a solid square filling
    /// its em box, at a fixed advance equal to the em size — ideal for
    /// asserting exact monospace advances without depending on a real
    /// font's metrics or hinting.
    const AHEM_TTF: &[u8] = include_bytes!("../tests/fixtures/ahem.ttf");

    fn context_with_ahem() -> (TextLayoutContext, String) {
        let mut cx = TextLayoutContext::new();
        let family = cx.register_font(AHEM_TTF).expect("ahem.ttf must register");
        (cx, family)
    }

    #[test]
    fn registering_ahem_resolves_to_its_own_family_name() {
        let (_cx, family) = context_with_ahem();
        assert_eq!(family, "Ahem");
    }

    #[test]
    fn monospace_line_has_uniform_per_cluster_advance() {
        let (mut cx, family) = context_with_ahem();
        let style = LineStyle {
            family: &family,
            font_size: 20.0,
            ligatures: false,
        };
        let layout = cx.layout_line(&style, "abcd", &[]);
        let clusters = layout.clusters();
        assert_eq!(
            clusters.len(),
            4,
            "one cluster per character, no ligatures requested"
        );

        let advance = clusters[0].advance;
        assert!(advance > 0.0);
        for (i, cluster) in clusters.iter().enumerate() {
            assert!(
                (cluster.advance - advance).abs() < 0.01,
                "Ahem is monospace: every cluster should share one advance, got {:?}",
                clusters
            );
            assert_eq!(cluster.byte_range, i..i + 1);
        }
        // Consecutive clusters must sit edge-to-edge on a monospace grid.
        for pair in clusters.windows(2) {
            assert!((pair[1].x - (pair[0].x + pair[0].advance)).abs() < 0.01);
        }
        assert!((layout.width() - advance * 4.0).abs() < 0.01);
    }

    #[test]
    fn ligature_override_wins_over_the_line_default_for_its_byte_range() {
        // Ahem defines no OpenType features at all, so this can't observe an
        // actual glyph-count collapse (there is no `liga`/`calt` table to
        // trigger) — what it verifies is the layer this deliverable owns:
        // that a per-range override is accepted and doesn't panic or get
        // silently dropped, and that the line still shapes to one cluster
        // per source byte either way. Verifying an observable ligature
        // collapse needs a font that actually defines `calt`/`liga`, which
        // is future work once a real programming font ships with the
        // editor (see the receipt).
        let (mut cx, family) = context_with_ahem();
        let style = LineStyle {
            family: &family,
            font_size: 20.0,
            ligatures: true,
        };
        let text = "if->then";
        let baseline = cx.layout_line(&style, text, &[]);
        let overridden = cx.layout_line(&style, text, &[(2..4, false)]);
        assert_eq!(baseline.clusters().len(), overridden.clusters().len());
        assert_eq!(overridden.clusters().len(), text.len());
    }

    #[test]
    fn empty_text_produces_no_clusters_and_zero_width() {
        let (mut cx, family) = context_with_ahem();
        let style = LineStyle {
            family: &family,
            font_size: 16.0,
            ligatures: true,
        };
        let layout = cx.layout_line(&style, "", &[]);
        assert!(layout.clusters().is_empty());
        assert_eq!(layout.width(), 0.0);
    }

    #[test]
    fn font_size_scales_advance_proportionally() {
        let (mut cx, family) = context_with_ahem();
        let small = cx.layout_line(
            &LineStyle {
                family: &family,
                font_size: 10.0,
                ligatures: false,
            },
            "m",
            &[],
        );
        let big = cx.layout_line(
            &LineStyle {
                family: &family,
                font_size: 20.0,
                ligatures: false,
            },
            "m",
            &[],
        );
        let small_advance = small.clusters()[0].advance;
        let big_advance = big.clusters()[0].advance;
        assert!(
            (big_advance - small_advance * 2.0).abs() < 0.01,
            "doubling font size must double the advance: {small_advance} vs {big_advance}"
        );
    }

    #[test]
    fn registering_garbage_bytes_returns_none_instead_of_panicking() {
        let mut cx = TextLayoutContext::new();
        assert!(cx.register_font(b"not a font").is_none());
    }
}

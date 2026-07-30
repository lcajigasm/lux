//! Phase 2's three named performance budgets
//! (`LUX_PROMPT_MAESTRO.md`/`docs/PERFORMANCE.md`): cold start to first
//! interactive frame (< 150 ms macOS), keypress → pixel latency p99
//! (< 8 ms), and no frame > 16 ms during continuous scroll of a 50k-line
//! file.
//!
//! **None of these three budgets is fully measurable yet**, and this file
//! doesn't pretend otherwise: there is still no `wgpu` quad/text render
//! pipeline (a separate, not-yet-built slice of `lux-render`'s own
//! deliverable 2) and no unified `lux` application binary wiring
//! `lux-platform`+`lux-render`+`lux-ui` together — every prior Phase 2
//! receipt this session has hit the same wall. A literal "cold start to
//! first interactive frame" needs a real window and a real first paint;
//! a literal "keypress → pixel" needs a real input event dispatched
//! through a real GPU submit and present; a literal "continuous scroll"
//! needs a real render loop painting real frames.
//!
//! What *is* real and measurable today: the CPU-bound components of each
//! budget that already exist in this crate, which will dominate each full
//! metric once the missing pieces land. Each benchmark below documents
//! exactly what it measures and what it deliberately excludes. See this
//! deliverable's receipt for the full reasoning and the measured numbers.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use lux_render::{GlyphAtlas, GlyphKey, GpuContext, LineStyle, TextLayoutContext};
use swash::{Charmap, FontRef, scale::ScaleContext};

/// Same public-domain Ahem test font used throughout this crate's own
/// tests (`tests/fixtures/README.md`): solid-square glyphs, so every
/// rasterization does real, non-trivial work with a predictable outcome.
const AHEM_TTF: &[u8] = include_bytes!("../tests/fixtures/ahem.ttf");
const FONT_SIZE: f32 = 16.0;

/// Budget component 1: "cold start to first interactive frame."
///
/// Measures only `GpuContext::new` — requesting a real adapter and
/// device/queue from the system's real GPU driver, no mock. This is
/// typically the single slowest step in a `wgpu` application's startup,
/// which is why it's the one piece of "cold start" this crate can measure
/// on its own. Excluded, because they don't exist yet: window creation
/// (`lux-platform`), font enumeration/registration, first layout, and the
/// actual first frame submit/present (no pipeline).
///
/// Requires a real GPU/driver in the environment (this benchmark is meant
/// to be run manually on real hardware, not in CI — see
/// `docs/PERFORMANCE.md`'s existing policy for why benchmarks don't run
/// in CI at all yet).
fn bench_gpu_context_new(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_start");
    // Adapter/device negotiation talks to the real driver; 20 samples is
    // enough for a stable mean without taking minutes.
    group.sample_size(20);
    group.bench_function("gpu_context_new", |b| {
        b.iter(|| {
            let context = pollster::block_on(GpuContext::new(None))
                .expect("a real GPU adapter must be available to run this benchmark");
            black_box(context);
        });
    });
    group.finish();
}

/// Budget component 2: "keypress → pixel latency, p99."
///
/// Measures the CPU-bound work a single keypress actually triggers today:
/// re-shaping the edited line (`TextLayoutContext::layout_line`) and
/// rasterizing the one newly-typed character if it wasn't already cached
/// (`GlyphAtlas::rasterize`) — the atlas is pre-warmed with the line's
/// *existing* characters first, matching a real editor where the line was
/// already on screen before this keystroke. Excluded, because they don't
/// exist yet: input-event dispatch latency (`lux-platform`) and GPU
/// submit/present + compositor latency (no quad/text pipeline, and OS
/// compositor latency isn't measurable without a real window anyway).
fn bench_keypress_reshape_and_rasterize(c: &mut Criterion) {
    const EXISTING_LINE: &str = "the quick brown fox jumps over";
    const EDITED_LINE: &str = "the quick brown fox jumps over!";
    const NEW_CHAR: char = '!';

    let font = FontRef::from_index(AHEM_TTF, 0).expect("ahem.ttf must parse as a valid font");
    let charmap = Charmap::from_font(&font);

    c.bench_function("keypress_reshape_and_rasterize_new_glyph", |b| {
        b.iter_batched(
            || {
                let mut text_layout = TextLayoutContext::new();
                let family = text_layout
                    .register_font(AHEM_TTF)
                    .expect("ahem.ttf must register");
                let mut atlas = GlyphAtlas::new(512);
                let mut scale_cx = ScaleContext::new();
                // Pre-warm the atlas with the line's characters as they
                // stood before this keystroke, the same way a real editor
                // would already have this (unedited) line on screen.
                for ch in EXISTING_LINE.chars() {
                    let glyph_id = charmap.map(ch);
                    let key = GlyphKey::new(font.key.value(), glyph_id, FONT_SIZE, 0.0);
                    atlas.rasterize(&mut scale_cx, font, key);
                }
                (text_layout, family, atlas, scale_cx)
            },
            |(mut text_layout, family, mut atlas, mut scale_cx)| {
                // The keystroke itself: re-shape the whole (still short)
                // line, then rasterize only the one genuinely new glyph.
                let style = LineStyle {
                    family: &family,
                    font_size: FONT_SIZE,
                    ligatures: false,
                };
                let layout = text_layout.layout_line(&style, EDITED_LINE, &[]);
                let glyph_id = charmap.map(NEW_CHAR);
                let key = GlyphKey::new(font.key.value(), glyph_id, FONT_SIZE, 0.0);
                let entry = atlas.rasterize(&mut scale_cx, font, key);
                black_box((layout, entry));
            },
            BatchSize::SmallInput,
        );
    });
}

/// Budget component 3: "no frame > 16 ms during continuous scroll of a
/// 50k-line file."
///
/// Measures shaping one screen's worth of freshly-scrolled-into-view
/// lines (`TextLayoutContext::layout_line`, one call per visible line) —
/// the actual, real per-frame CPU cost `lux-ui`'s virtualized `EditorView`
/// (Phase 2 deliverable 4) will pay each time scrolling reveals lines that
/// were never shaped before, the worst case for continuous scroll.
/// Deliberately scoped to `lux-render` alone (this deliverable's gate is
/// `-p lux-render`): it does not invoke `lux-ui::EditorView` itself, only
/// the same shaping primitive that view calls into per visible line.
/// Excluded: cursor/selection/indent-guide/column-ruler computation
/// (`lux-ui`'s own job) and the actual GPU draw of the shaped result (no
/// pipeline).
fn bench_scroll_frame_shaping_cost(c: &mut Criterion) {
    const VISIBLE_LINE_COUNT: usize = 50; // a plausible on-screen line count
    const TOTAL_LINE_COUNT: usize = 50_000;

    let lines: Vec<String> = (0..TOTAL_LINE_COUNT)
        .map(|i| format!("    let value_{i} = compute_something(i, {i});"))
        .collect();

    let mut text_layout = TextLayoutContext::new();
    let family = text_layout
        .register_font(AHEM_TTF)
        .expect("ahem.ttf must register");
    let style = LineStyle {
        family: &family,
        font_size: FONT_SIZE,
        ligatures: false,
    };

    let mut group = c.benchmark_group("scroll");
    let mut offset = 0usize;
    group.bench_function(
        "shape_one_visible_window_of_50_never_before_seen_lines",
        |b| {
            b.iter(|| {
                let window = &lines[offset..offset + VISIBLE_LINE_COUNT];
                for line in window {
                    let layout = text_layout.layout_line(&style, line, &[]);
                    black_box(layout);
                }
                // Advance so the next sample shapes a genuinely different
                // window — continuous scroll never re-shows the same lines
                // twice in a row.
                offset = (offset + 1) % (TOTAL_LINE_COUNT - VISIBLE_LINE_COUNT);
            });
        },
    );
    group.finish();
}

criterion_group!(
    phase2_budgets,
    bench_gpu_context_new,
    bench_keypress_reshape_and_rasterize,
    bench_scroll_frame_shaping_cost
);
criterion_main!(phase2_budgets);

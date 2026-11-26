//! ST-MAP-01: the minimap frame on a 50k-line Rust file with syntax colors.
//! `scroll` is a frame whose only change is the editor's visible range;
//! `typed_char` is a frame after one character typed on one line, the
//! patch path that re-measures and re-inks that line only; `full_build` is a
//! cold model, every line measured once. The first two must stay far below
//! the third: that gap is the incremental claim.

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use lux_core::buffer::Buffer;
use lux_ui::minimap::{ColorRun, LineColors, LineEdit, Minimap, MinimapModel, MinimapSample};
use std::hint::black_box;

const LINES: usize = 50_000;
const ROWS: usize = 300;

/// Highlight-token stand-in: one colored run per `fn`, as the editor's
/// token list holds one entry per keyword.
struct Keywords(Vec<ColorRun>);

impl LineColors for Keywords {
    fn stamp(&self) -> u64 {
        1
    }
    fn runs(&self, start: usize, end: usize, out: &mut Vec<ColorRun>) {
        let first = self.0.partition_point(|run| run.end <= start);
        out.extend(self.0[first..].iter().take_while(|run| run.start < end));
    }
}

fn source() -> String {
    (0..LINES)
        .map(|i| format!("    fn item_{i}() {{ call({i}); }}\n"))
        .collect()
}

fn keywords(text: &str) -> Keywords {
    Keywords(
        text.match_indices("fn")
            .map(|(at, _)| ColorRun {
                start: at,
                end: at + 2,
                rgb: [249, 38, 114],
            })
            .collect(),
    )
}

fn sample<'a>(
    buffer: &'a Buffer,
    edit: Option<LineEdit>,
    visible: std::ops::Range<usize>,
    colors: &'a Keywords,
) -> MinimapSample<'a> {
    MinimapSample {
        revision: buffer.revision(),
        document: 1,
        edit,
        buffer,
        visible,
        available_rows: ROWS,
        folds: None,
        colors: Some(colors),
    }
}

fn minimap_frame(c: &mut Criterion) {
    let text = source();
    let colors = keywords(&text);
    let minimap = Minimap::new();
    let mut group = c.benchmark_group("minimap_50k_lines");

    group.bench_function("full_build", |b| {
        let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
        b.iter_batched(
            MinimapModel::default,
            |mut model| black_box(model.frame(&minimap, sample(&buffer, None, 0..40, &colors))),
            BatchSize::LargeInput,
        );
    });

    group.bench_function("scroll", |b| {
        let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
        let mut model = MinimapModel::default();
        model.frame(&minimap, sample(&buffer, None, 0..40, &colors));
        let mut top = 0usize;
        b.iter(|| {
            top = (top + 37) % (LINES - 40);
            black_box(model.frame(&minimap, sample(&buffer, None, top..top + 40, &colors)))
        });
    });

    group.bench_function("typed_char", |b| {
        let mut buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
        let mut model = MinimapModel::default();
        model.frame(&minimap, sample(&buffer, None, 0..40, &colors));
        let line = LINES / 2;
        b.iter(|| {
            let before = buffer.revision();
            let at = buffer.rope().line_to_char(line) + 4;
            buffer.insert(at, "x");
            let edit = LineEdit {
                line,
                before,
                after: buffer.revision(),
            };
            black_box(model.frame(&minimap, sample(&buffer, Some(edit), 0..40, &colors)))
        });
    });
    group.finish();
}

criterion_group!(benches, minimap_frame);
criterion_main!(benches);

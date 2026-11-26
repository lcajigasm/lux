//! ST-NAV-03's budget: with the project index already built, locating a
//! symbol in `Cmd+Shift+R` takes < 30 ms. This measures the work one
//! keystroke does on the UI thread: ranking a published
//! `lux_workspace::SymbolSnapshot` of 50,000 symbols (500 files x 100
//! functions, roughly a 50k-line project or larger). Building the snapshot
//! happens on the indexing thread and is measured separately, since the
//! panel never waits for it.

use criterion::{Criterion, criterion_group, criterion_main};
use lux_workspace::{SymbolIndex, SymbolSnapshot};
use std::hint::black_box;
use std::path::PathBuf;

const FILES: usize = 500;
const FUNCTIONS_PER_FILE: usize = 100;

fn make_index() -> SymbolIndex {
    let mut index = SymbolIndex::new();
    for f in 0..FILES {
        let mut source = String::new();
        for i in 0..FUNCTIONS_PER_FILE {
            source.push_str(&format!(
                "pub fn handle_request_{f}_{i}(x: u32) -> u32 {{ x }}\n"
            ));
        }
        let path = PathBuf::from(format!("src/module_{:03}/file_{f:03}.rs", f % 20));
        index.index_file_content(&path, &source, None);
    }
    assert_eq!(index.len(), FILES * FUNCTIONS_PER_FILE);
    index
}

fn bench_project_symbols(c: &mut Criterion) {
    let index = make_index();
    let snapshot = SymbolSnapshot::from_index(&index, false);
    let mut group = c.benchmark_group("project_symbols_50k");
    // A query every symbol matches: the worst case for ranking.
    group.bench_function("filter_all_match", |b| {
        b.iter(|| black_box(snapshot.search(black_box("request"), 200).len()))
    });
    group.bench_function("filter_one_match", |b| {
        b.iter(|| {
            black_box(
                snapshot
                    .search(black_box("handle_request_250_42"), 200)
                    .len(),
            )
        })
    });
    group.bench_function("filter_file_at_symbol", |b| {
        b.iter(|| {
            black_box(
                snapshot
                    .search(black_box("module_007@request_7_"), 200)
                    .len(),
            )
        })
    });
    group.bench_function("empty_query", |b| {
        b.iter(|| black_box(snapshot.search(black_box(""), 200).len()))
    });
    group.bench_function("build_snapshot_off_ui_thread", |b| {
        b.iter(|| black_box(SymbolSnapshot::from_index(&index, false).len()))
    });
    group.finish();
}

criterion_group!(benches, bench_project_symbols);
criterion_main!(benches);

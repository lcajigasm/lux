//! Phase 4's named budget (`LUX_PROMPT_MAESTRO.md`'s acceptance criteria
//! for the "Find in Files" deliverable): searching 1 GB of code in < 2 s.
//! A real, on-disk 1 GB corpus — many files spread across subdirectories,
//! not one giant file, since that's the actual shape
//! `ignore::WalkParallel`'s directory-fanning parallelism is for and what
//! a real project looks like — generated once per `cargo bench` run (not
//! per sample; regenerating it for every one of criterion's 10 samples
//! would make this benchmark impractically slow) and removed afterward so
//! no 1 GB fixture lingers on disk between runs.

use criterion::{Criterion, criterion_group, criterion_main};
use lux_search::{SearchOptions, find_in_files};
use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};

const TARGET_BYTES: usize = 1024 * 1024 * 1024;
const FILE_COUNT: usize = 2_000;
const FILE_BYTES: usize = TARGET_BYTES / FILE_COUNT;
const SUBDIR_COUNT: usize = 20;
const NEEDLE: &str = "LUX_BENCH_NEEDLE_4f8e";

fn corpus_dir() -> PathBuf {
    std::env::temp_dir().join("lux-benches-find-in-files-1gb")
}

/// Writes `FILE_COUNT` files of ~`FILE_BYTES` each under `dir`, spread
/// across `SUBDIR_COUNT` subdirectories. Every 500th line of every file is
/// `NEEDLE`, so the search has real matches to find and collect across the
/// whole tree, not just files to rule out.
fn make_1gb_corpus(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    let filler = "the quick brown fox jumps over the lazy dog in this line of pretend code\n";
    let needle_line = format!("// {NEEDLE}\n");
    let lines_per_file = FILE_BYTES / filler.len();

    for i in 0..FILE_COUNT {
        let subdir = dir.join(format!("pkg_{:03}", i % SUBDIR_COUNT));
        fs::create_dir_all(&subdir).unwrap();
        let mut content = String::with_capacity(FILE_BYTES + filler.len());
        for line in 0..lines_per_file {
            if line % 500 == 0 {
                content.push_str(&needle_line);
            } else {
                content.push_str(filler);
            }
        }
        fs::write(subdir.join(format!("file_{i:05}.rs")), content).unwrap();
    }
}

fn bench_find_in_files_1gb(c: &mut Criterion) {
    let dir = corpus_dir();
    fs::remove_dir_all(&dir).ok();
    make_1gb_corpus(&dir);

    let mut group = c.benchmark_group("find_in_files");
    // A search over a real 1 GB tree, run 10 times (criterion's minimum
    // sample size), is already tens of seconds — the same reasoning
    // `phase1_budgets.rs` uses for its own 100 MB benchmark.
    group.sample_size(10);
    group.bench_function("search_1gb_corpus", |b| {
        b.iter(|| {
            let mut stream = find_in_files(&dir, NEEDLE, &SearchOptions::default()).unwrap();
            stream.wait_until_done();
            black_box(stream.matches().len());
        });
    });
    group.finish();

    fs::remove_dir_all(&dir).ok();
}

criterion_group!(budgets, bench_find_in_files_1gb);
criterion_main!(budgets);

// Harness smoke test: proves the criterion pipeline (run, save baseline,
// regression-compare) works end to end. Replace with real budget benchmarks
// (see docs/PERFORMANCE.md) as each phase lands the capability it measures.
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

fn bench_smoke(c: &mut Criterion) {
    c.bench_function("smoke_fibonacci_20", |b| {
        b.iter(|| fibonacci(black_box(20)))
    });
}

criterion_group!(benches, bench_smoke);
criterion_main!(benches);

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use helix::pricing::TernarySearchOptimizer;

fn bench_ternary_search(c: &mut Criterion) {
    let optimizer = TernarySearchOptimizer::default();
    let eval = |x: u128| -> Option<u128> {
        let diff = x.abs_diff(500);
        let profit = 250_000u128.saturating_sub(diff * diff);
        Some(x + profit)
    };

    c.bench_function("ternary_search", |b| {
        b.iter(|| optimizer.find_optimal(black_box(0), black_box(1000), &eval))
    });
}

criterion_group!(benches, bench_ternary_search);
criterion_main!(benches);

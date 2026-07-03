//! Benchmarks for the deadwood solver

use core::hint::black_box;
use criterion::{Criterion, criterion_group, criterion_main};
use gin_rummy::{Hand, best_melds, deadwood};

fn solver(c: &mut Criterion) {
    let typical: Hand = "TQ.49.49J.456".parse().unwrap();
    let worst: Hand = "A23456789TJ...".parse().unwrap();
    let gin: Hand = "A23.456.789TJ.".parse().unwrap();

    c.bench_function("deadwood typical", |b| {
        b.iter(|| deadwood(black_box(typical)));
    });
    c.bench_function("deadwood 11-card run", |b| {
        b.iter(|| deadwood(black_box(worst)));
    });
    c.bench_function("best_melds big gin", |b| {
        b.iter(|| best_melds(black_box(gin)));
    });
}

criterion_group!(benches, solver);
criterion_main!(benches);

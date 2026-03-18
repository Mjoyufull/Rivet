use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use matrix_sdk_ui::eyeball_im::Vector;
use rivet_ui::perf::{changed_identity_ranges, expand_incremental_range};
use std::sync::Arc;

fn build_vector(len: usize) -> Vector<Arc<u64>> {
    let mut items = Vector::new();
    for ix in 0..len {
        items.push_back(Arc::new(ix as u64));
    }
    items
}

fn bench_changed_identity_ranges(c: &mut Criterion) {
    let base = build_vector(10_000);
    let mut group = c.benchmark_group("timeline_diff");

    group.bench_function("changed_identity_ranges_append", |b| {
        let previous = base.clone();
        let mut current = previous.clone();
        for ix in 0..32 {
            current.push_back(Arc::new((10_000 + ix) as u64));
        }

        b.iter(|| changed_identity_ranges(&previous, &current));
    });

    group.bench_function("changed_identity_ranges_mid_set", |b| {
        let previous = base.clone();
        let mut current = previous.clone();
        current.set(5_000, Arc::new(99_999));

        b.iter(|| changed_identity_ranges(&previous, &current));
    });

    group.bench_function("changed_identity_ranges_front_insert", |b| {
        let previous = base.clone();
        let mut current = previous.clone();
        for ix in 0..16 {
            current.insert(ix, Arc::new((20_000 + ix) as u64));
        }

        b.iter(|| changed_identity_ranges(&previous, &current));
    });

    for len in [1_024usize, 8_192, 32_768] {
        let mut counts = vec![1usize; len];
        for ix in (0..len).step_by(13) {
            counts[ix] = 0;
        }

        group.bench_with_input(
            BenchmarkId::new("expand_incremental_range", len),
            &counts,
            |b, counts| {
                b.iter(|| expand_incremental_range(counts, len / 3, (len / 3) + 2));
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_changed_identity_ranges);
criterion_main!(benches);

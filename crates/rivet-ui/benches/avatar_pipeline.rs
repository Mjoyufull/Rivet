use criterion::{Criterion, criterion_group, criterion_main};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use rivet_ui::perf::prepare_avatar_bytes_for_bench;
use std::io::Cursor;

fn sample_avatar_input() -> Vec<u8> {
    let mut image = RgbaImage::from_pixel(1024, 768, Rgba([0, 0, 0, 0]));

    for y in 96..672 {
        for x in 160..864 {
            let alpha = if (x + y) % 5 == 0 { 220 } else { 255 };
            image.put_pixel(x, y, Rgba([45, 116, 220, alpha]));
        }
    }

    let mut encoded = Vec::new();
    DynamicImage::ImageRgba8(image)
        .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Png)
        .expect("encode avatar sample");
    encoded
}

fn bench_avatar_pipeline(c: &mut Criterion) {
    let input = sample_avatar_input();
    c.bench_function("prepare_avatar_bytes", |b| {
        b.iter(|| prepare_avatar_bytes_for_bench(&input).expect("prepare avatar bytes"));
    });
}

criterion_group!(benches, bench_avatar_pipeline);
criterion_main!(benches);

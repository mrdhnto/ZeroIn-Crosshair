#![cfg(target_os = "linux")]

use criterion::{black_box, Criterion, BatchSize};
use ZeroIn::canvas::{self, SwCanvas};

pub fn bench_swcanvas_new(c: &mut Criterion) {
    let mut group = c.benchmark_group("SwCanvas new");

    group.bench_function("1920x1080", |b| {
        b.iter_with_setup(|| SwCanvas::new(black_box(1920), black_box(1080)), |c| c)
    });
    group.bench_function("256x256", |b| {
        b.iter_with_setup(|| SwCanvas::new(black_box(256), black_box(256)), |c| c)
    });

    group.finish();
}

pub fn bench_premul_pixel(c: &mut Criterion) {
    let mut group = c.benchmark_group("premul_pixel");

    group.bench_function("opaque", |b| {
        b.iter(|| canvas::premul_pixel(black_box((0.5, 0.5, 0.5, 1.0))))
    });
    group.bench_function("translucent", |b| {
        b.iter(|| canvas::premul_pixel(black_box((1.0, 0.0, 0.0, 0.5))))
    });
    group.bench_function("transparent", |b| {
        b.iter(|| canvas::premul_pixel(black_box((0.0, 0.0, 0.0, 0.0))))
    });

    group.finish();
}

pub fn bench_blend_premul(c: &mut Criterion) {
    let mut group = c.benchmark_group("blend_premul");

    group.bench_function("opaque on opaque", |b| {
        b.iter(|| canvas::blend_premul(black_box(0xFF_FF_FF_FF), black_box(0xFF_00_00_00)))
    });
    group.bench_function("translucent on opaque", |b| {
        b.iter(|| canvas::blend_premul(black_box(0xFF_FF_FF_FF), black_box(0x80_FF_00_00)))
    });
    group.bench_function("transparent on opaque", |b| {
        b.iter(|| canvas::blend_premul(black_box(0xFF_FF_FF_FF), black_box(0x00_FF_00_00)))
    });

    group.finish();
}

pub fn bench_rotate_buffer(c: &mut Criterion) {
    let mut group = c.benchmark_group("rotate_buffer");

    let small: Vec<u32> = vec![0xFF_00_00_00; 32 * 32];
    let medium: Vec<u32> = vec![0xFF_00_00_00; 128 * 128];

    group.bench_function("32x32 45deg", |b| {
        b.iter(|| canvas::rotate_buffer(black_box(&small), 32, 32, 0.7854, 16.0, 16.0))
    });
    group.bench_function("128x128 45deg", |b| {
        b.iter(|| canvas::rotate_buffer(black_box(&medium), 128, 128, 0.7854, 64.0, 64.0))
    });

    group.finish();
}

pub fn bench_fill_rect(c: &mut Criterion) {
    let mut group = c.benchmark_group("SwCanvas fill_rect");

    let color = (1.0, 0.0, 0.0, 0.85);

    group.bench_function("fullscreen (1920x1080)", |b| {
        b.iter_batched_ref(
            || SwCanvas::new(1920, 1080),
            |canvas| canvas.fill_rect(black_box(0.0), black_box(0.0), black_box(1920.0), black_box(1080.0), black_box(color)),
            BatchSize::SmallInput,
        )
    });
    group.bench_function("small rect (100x100)", |b| {
        b.iter_batched_ref(
            || SwCanvas::new(1920, 1080),
            |canvas| canvas.fill_rect(black_box(100.0), black_box(100.0), black_box(200.0), black_box(200.0), black_box(color)),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

pub fn bench_fill_ellipse(c: &mut Criterion) {
    let mut group = c.benchmark_group("SwCanvas fill_ellipse");

    let color = (0.0, 1.0, 0.0, 0.85);

    group.bench_function("large radius 200", |b| {
        b.iter_batched_ref(
            || SwCanvas::new(1920, 1080),
            |canvas| canvas.fill_ellipse(black_box(960.0), black_box(540.0), black_box(200.0), black_box(200.0), black_box(color)),
            BatchSize::SmallInput,
        )
    });
    group.bench_function("small radius 20", |b| {
        b.iter_batched_ref(
            || SwCanvas::new(1920, 1080),
            |canvas| canvas.fill_ellipse(black_box(960.0), black_box(540.0), black_box(20.0), black_box(20.0), black_box(color)),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

pub fn bench_draw_line(c: &mut Criterion) {
    let mut group = c.benchmark_group("SwCanvas draw_line");

    let color = (0.0, 0.0, 1.0, 0.85);

    group.bench_function("long diagonal", |b| {
        b.iter_batched_ref(
            || SwCanvas::new(1920, 1080),
            |canvas| canvas.draw_line(black_box(0.0), black_box(0.0), black_box(1919.0), black_box(1079.0), black_box(2.0), black_box(color)),
            BatchSize::SmallInput,
        )
    });
    group.bench_function("short horizontal", |b| {
        b.iter_batched_ref(
            || SwCanvas::new(1920, 1080),
            |canvas| canvas.draw_line(black_box(100.0), black_box(100.0), black_box(200.0), black_box(100.0), black_box(2.0), black_box(color)),
            BatchSize::SmallInput,
        )
    });
    group.bench_function("point (no-op)", |b| {
        b.iter_batched_ref(
            || SwCanvas::new(1920, 1080),
            |canvas| canvas.draw_line(black_box(100.0), black_box(100.0), black_box(100.0), black_box(100.0), black_box(2.0), black_box(color)),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

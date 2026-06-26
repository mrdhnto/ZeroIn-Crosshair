#![cfg(target_os = "linux")]

use criterion::{black_box, Criterion, BatchSize};
use ZeroIn::canvas::{self, SwCanvas};
use ZeroIn::config::CrosshairType;
use ZeroIn::crosshair;

const COLOR: canvas::Color = (1.0, 0.0, 0.0, 0.85);
const BORDER_COLOR: canvas::Color = (0.0, 0.0, 0.0, 0.85);

fn make_canvas() -> SwCanvas {
    SwCanvas::new(1920, 1080)
}

pub fn bench_draw_cross(c: &mut Criterion) {
    let mut group = c.benchmark_group("Crosshair draw_cross");

    group.bench_function("default", |b| {
        b.iter_batched_ref(
            make_canvas,
            |canvas| crosshair::draw(
                canvas,
                black_box(COLOR),
                black_box(Some(BORDER_COLOR)),
                black_box(CrosshairType::Cross),
                black_box(960.0), black_box(540.0),
                black_box(24.0), black_box(2.0), black_box(2.0),
                black_box(true), black_box(true),
                black_box(1.0), black_box(2.0), black_box(0.0),
                black_box(1.5),
            ),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

pub fn bench_draw_dot(c: &mut Criterion) {
    let mut group = c.benchmark_group("Crosshair draw_dot");

    group.bench_function("radius 12", |b| {
        b.iter_batched_ref(
            make_canvas,
            |canvas| crosshair::draw(
                canvas,
                black_box(COLOR),
                black_box(Some(BORDER_COLOR)),
                black_box(CrosshairType::Dot),
                black_box(960.0), black_box(540.0),
                black_box(24.0), black_box(2.0), black_box(2.0),
                black_box(true), black_box(true),
                black_box(1.0), black_box(0.0), black_box(0.0),
                black_box(1.5),
            ),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

pub fn bench_draw_circle(c: &mut Criterion) {
    let mut group = c.benchmark_group("Crosshair draw_circle");

    group.bench_function("radius 12 outline", |b| {
        b.iter_batched_ref(
            make_canvas,
            |canvas| crosshair::draw(
                canvas,
                black_box(COLOR),
                black_box(Some(BORDER_COLOR)),
                black_box(CrosshairType::Circle),
                black_box(960.0), black_box(540.0),
                black_box(24.0), black_box(2.0), black_box(2.0),
                black_box(true), black_box(true),
                black_box(1.0), black_box(2.0), black_box(0.0),
                black_box(1.5),
            ),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

pub fn bench_draw_arrow(c: &mut Criterion) {
    let mut group = c.benchmark_group("Crosshair draw_arrow");

    group.bench_function("default", |b| {
        b.iter_batched_ref(
            make_canvas,
            |canvas| crosshair::draw(
                canvas,
                black_box(COLOR),
                black_box(Some(BORDER_COLOR)),
                black_box(CrosshairType::Arrow),
                black_box(960.0), black_box(540.0),
                black_box(24.0), black_box(2.0), black_box(2.0),
                black_box(true), black_box(true),
                black_box(1.0), black_box(2.0), black_box(0.0),
                black_box(1.5),
            ),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

pub fn bench_draw_t(c: &mut Criterion) {
    let mut group = c.benchmark_group("Crosshair draw_t");

    group.bench_function("default", |b| {
        b.iter_batched_ref(
            make_canvas,
            |canvas| crosshair::draw(
                canvas,
                black_box(COLOR),
                black_box(Some(BORDER_COLOR)),
                black_box(CrosshairType::T),
                black_box(960.0), black_box(540.0),
                black_box(24.0), black_box(2.0), black_box(2.0),
                black_box(true), black_box(true),
                black_box(1.0), black_box(2.0), black_box(0.0),
                black_box(1.5),
            ),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

pub fn bench_draw_diamond(c: &mut Criterion) {
    let mut group = c.benchmark_group("Crosshair draw_diamond");

    group.bench_function("default", |b| {
        b.iter_batched_ref(
            make_canvas,
            |canvas| crosshair::draw(
                canvas,
                black_box(COLOR),
                black_box(Some(BORDER_COLOR)),
                black_box(CrosshairType::Diamond),
                black_box(960.0), black_box(540.0),
                black_box(24.0), black_box(2.0), black_box(2.0),
                black_box(true), black_box(true),
                black_box(1.0), black_box(2.0), black_box(0.0),
                black_box(1.5),
            ),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

#[cfg(target_os = "linux")]
mod canvas_bench;
#[cfg(target_os = "linux")]
mod crosshair_bench;
mod config_bench;
mod profiles_bench;

use criterion::{criterion_group, criterion_main};

use config_bench::*;
use profiles_bench::*;

#[cfg(target_os = "linux")]
use canvas_bench::*;
#[cfg(target_os = "linux")]
use crosshair_bench::*;

criterion_group!(
    benches,
    bench_hotkey_parse,
    bench_hotkey_from_parts,
    bench_crosshair_type_from_str,
    bench_crosshair_type_as_str,
    bench_parse_color,
    bench_serialize,
    bench_deserialize,
);

#[cfg(target_os = "linux")]
criterion_group!(
    canvas_benches,
    bench_swcanvas_new,
    bench_premul_pixel,
    bench_blend_premul,
    bench_rotate_buffer,
    bench_fill_rect,
    bench_fill_ellipse,
    bench_draw_line,
);

#[cfg(target_os = "linux")]
criterion_group!(
    crosshair_benches,
    bench_draw_cross,
    bench_draw_dot,
    bench_draw_circle,
    bench_draw_arrow,
    bench_draw_t,
    bench_draw_diamond,
);

#[cfg(target_os = "linux")]
criterion_main!(benches, canvas_benches, crosshair_benches);

#[cfg(not(target_os = "linux"))]
criterion_main!(benches);

use criterion::{black_box, Criterion};

use ZeroIn::config::{self, CrosshairType, Hotkey};

pub fn bench_hotkey_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("Hotkey parsing");

    group.bench_function("ctrl+backslash", |b| {
        b.iter(|| Hotkey::parse(black_box("CTRL+\\")))
    });
    group.bench_function("alt+space", |b| {
        b.iter(|| Hotkey::parse(black_box("ALT+SPACE")))
    });
    group.bench_function("shift+f1", |b| {
        b.iter(|| Hotkey::parse(black_box("SHIFT+F1")))
    });
    group.bench_function("ctrl+alt+delete", |b| {
        b.iter(|| Hotkey::parse(black_box("CTRL+ALT+DELETE")))
    });
    group.bench_function("win+shift+s", |b| {
        b.iter(|| Hotkey::parse(black_box("WIN+SHIFT+S")))
    });

    group.finish();
}

pub fn bench_hotkey_from_parts(c: &mut Criterion) {
    let mut group = c.benchmark_group("Hotkey from_parts");

    group.bench_function("ctrl+backslash", |b| {
        b.iter(|| Hotkey::from_parts(black_box("CTRL"), black_box("\\")))
    });

    group.finish();
}

pub fn bench_crosshair_type_from_str(c: &mut Criterion) {
    let mut group = c.benchmark_group("CrosshairType from_str");

    for &name in &["cross", "dot", "circle", "diamond", "arrow", "t", "unknown"] {
        group.bench_function(name, |b| b.iter(|| CrosshairType::from_str(black_box(name))));
    }

    group.finish();
}

pub fn bench_crosshair_type_as_str(c: &mut Criterion) {
    let mut group = c.benchmark_group("CrosshairType as_str");

    for ct in [
        CrosshairType::Cross,
        CrosshairType::Dot,
        CrosshairType::Circle,
        CrosshairType::Diamond,
        CrosshairType::Arrow,
        CrosshairType::T,
    ] {
        group.bench_function(format!("{ct:?}"), |b| b.iter(|| CrosshairType::as_str(&ct)));
    }

    group.finish();
}

pub fn bench_parse_color(c: &mut Criterion) {
    let mut group = c.benchmark_group("Config parse_color");

    let cfg_red = {
        let mut c = config::Config::default();
        c.color_hex = "#FF0000".into();
        c
    };
    let cfg_custom = {
        let mut c = config::Config::default();
        c.color_hex = "#AABBCC".into();
        c
    };

    group.bench_function("red", |b| b.iter(|| cfg_red.parse_color()));
    group.bench_function("custom", |b| b.iter(|| cfg_custom.parse_color()));

    group.finish();
}

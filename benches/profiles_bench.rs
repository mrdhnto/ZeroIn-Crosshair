use criterion::{black_box, Criterion};

use ZeroIn::profiles::Profile;

fn sample_profiles() -> Vec<Profile> {
    vec![
        Profile {
            name: "Default".into(),
            game_exe: None,
            crosshair_type: "cross".into(),
            size: 24.0,
            thickness: 2.0,
            thickness_h: 2.0,
            thickness_v: 2.0,
            color_hex: "#FF0000".into(),
            border_color_hex: "#000000".into(),
            dot_center: true,
            opacity: 0.85,
            border: true,
            border_size: 1.0,
            space_width: 2.0,
            rotation: 0.0,
            dot_size: 1.5,
            png_crosshair: None,
            mirror_crosshair: false,
            set_monitor: 0,
            adjust_x: 0.0,
            adjust_y: 0.0,
        },
        Profile {
            name: "CS2".into(),
            game_exe: Some("cs2.exe".into()),
            crosshair_type: "cross".into(),
            size: 18.0,
            thickness: 1.5,
            thickness_h: 1.5,
            thickness_v: 1.5,
            color_hex: "#00FF00".into(),
            border_color_hex: "#000000".into(),
            dot_center: false,
            opacity: 1.0,
            border: false,
            border_size: 0.0,
            space_width: 1.0,
            rotation: 0.0,
            dot_size: 0.0,
            png_crosshair: None,
            mirror_crosshair: false,
            set_monitor: 0,
            adjust_x: 0.0,
            adjust_y: 0.0,
        },
        Profile {
            name: "Valorant".into(),
            game_exe: Some("VALORANT-Win64-Shipping.exe".into()),
            crosshair_type: "dot".into(),
            size: 10.0,
            thickness: 2.0,
            thickness_h: 2.0,
            thickness_v: 2.0,
            color_hex: "#00FF00".into(),
            border_color_hex: "#000000".into(),
            dot_center: true,
            opacity: 0.9,
            border: true,
            border_size: 1.0,
            space_width: 0.0,
            rotation: 0.0,
            dot_size: 2.0,
            png_crosshair: Some("custom.png".into()),
            mirror_crosshair: false,
            set_monitor: 0,
            adjust_x: 0.0,
            adjust_y: 0.0,
        },
    ]
}

pub fn bench_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("Profile serialize");

    let profiles = sample_profiles();

    group.bench_function("3 profiles to JSON", |b| {
        b.iter(|| serde_json::to_string_pretty(black_box(&profiles)))
    });

    group.finish();
}

pub fn bench_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("Profile deserialize");

    let profiles = sample_profiles();
    let json = serde_json::to_string_pretty(&profiles).unwrap();

    group.bench_function("3 profiles from JSON", |b| {
        b.iter(|| serde_json::from_str::<Vec<Profile>>(black_box(&json)))
    });

    group.finish();
}

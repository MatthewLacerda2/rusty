use crate::settings::quality::{Levels, QualityPresets, QualityValues};

/// `(preset, row, why)`: a seeded row allowed three equal levels.
const FLAT: &[(&str, &str, &str)] = &[];

fn flat<T: PartialEq>(l: &Levels<T>) -> bool {
    l.low == l.medium && l.medium == l.high
}

/// Names every field, so a row added to [`QualityValues`] fails to compile until it is here.
fn rows(values: &QualityValues) -> [(&'static str, bool); 7] {
    let QualityValues {
        ssao,
        ssil,
        ssr,
        bloom_resolution_divisor,
        depth_of_field_resolution_divisor,
        motion_blur_samples,
        volumetric_fog,
        ui_blur_divisor: _,
        shadowmask_mode: _,
        gpu_occlusion_culling: _,
        gpu_cluster_culling: _,
    } = values;
    [
        ("ssao", flat(ssao)),
        ("ssil", flat(ssil)),
        ("ssr", flat(ssr)),
        ("bloom_resolution_divisor", flat(bloom_resolution_divisor)),
        (
            "depth_of_field_resolution_divisor",
            flat(depth_of_field_resolution_divisor),
        ),
        ("motion_blur_samples", flat(motion_blur_samples)),
        ("volumetric_fog", flat(volumetric_fog)),
    ]
}

#[test]
fn every_seeded_row_differs_between_its_levels() {
    let (mut dead, mut stale) = (Vec::new(), Vec::new());
    for preset in &QualityPresets::default().presets {
        for (row, is_flat) in rows(&preset.values) {
            let excused = FLAT.iter().any(|(p, r, _)| *p == preset.name && *r == row);
            let name = format!("{}.{row}", preset.name);
            match (is_flat, excused) {
                (true, false) => dead.push(name),
                (false, true) => stale.push(name),
                _ => {}
            }
        }
    }
    assert!(dead.is_empty(), "flat rows (a dead knob): {dead:?}");
    assert!(stale.is_empty(), "FLAT entries no longer flat: {stale:?}");
}

#[test]
fn flat_means_all_three_levels_equal() {
    assert!(flat(&Levels {
        low: 2,
        medium: 2,
        high: 2
    }));
    assert!(!flat(&Levels {
        low: 4,
        medium: 3,
        high: 2
    }));
    assert!(!flat(&Levels {
        low: 2,
        medium: 2,
        high: 1
    }));
}

#[test]
fn every_seeded_preset_ships_the_gpu_cull_off() {
    for preset in &QualityPresets::default().presets {
        let v = &preset.values;
        assert!(
            !v.gpu_occlusion_culling && !v.gpu_cluster_culling,
            "{} seeds the GPU cull on",
            preset.name
        );
    }
    let engine = QualityValues::default();
    assert!(!engine.gpu_occlusion_culling && !engine.gpu_cluster_culling);
}

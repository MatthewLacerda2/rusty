//! Every seeded quality row moves between its levels (#1716): a row whose three
//! levels are equal is a knob that reaches nothing, as the `High` preset's bloom
//! was at 2/2/2.

use crate::core::settings::quality::{Levels, QualityPresets, QualityValues};

/// Rows meant to be flat, each `(preset, row, why)`; a row listed here may keep its
/// three levels equal. None is today.
const FLAT: &[(&str, &str, &str)] = &[];

fn flat<T: PartialEq>(l: &Levels<T>) -> bool {
    l.low == l.medium && l.medium == l.high
}

/// Each row of `values` and whether it is flat. The destructuring names every
/// field, so a row added to [`QualityValues`] fails to compile until it is here.
fn rows(values: &QualityValues) -> [(&'static str, bool); 7] {
    let QualityValues {
        ssao,
        ssil,
        ssr,
        bloom_resolution_divisor,
        depth_of_field_resolution_divisor,
        motion_blur_samples,
        volumetric_fog,
        // One value, not three levels: there is nothing to differ.
        ui_blur_divisor: _,
        shadowmask_mode: _,
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

/// Every row of every seeded preset differs between its levels, unless [`FLAT`]
/// gives it a reason; an entry there whose row is not flat is stale.
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

/// The check flags equal levels and passes ones that differ anywhere.
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

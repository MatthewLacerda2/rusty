//! Refraction (#1523, HDRP's *Planar* model): clear water with a rippled normal map
//! bends the checker floor under it; with refraction `None` the same water leaves
//! the floor where flat water leaves it. (The slab casts its own shadow on the
//! floor either way, so the baseline is flat water, not no water.)

use glam::Vec3;
use rusty::components::{MaterialAsset, RefractionModel};
use rusty::scene::Camera;

use super::{checker, differing, glass, ripples, shot, slab, stage};

/// A checker floor under a clear water slab refracting by `model`, rippled when
/// `rippled`.
fn pool(model: RefractionModel, rippled: bool) -> rusty::scene::Scene {
    let mut scene = stage(-1.2);
    let tiles = MaterialAsset {
        base_color_map: Some(checker("refraction_floor", 16).into()),
        roughness: 1.0,
        ..MaterialAsset::default()
    };
    slab(
        &mut scene,
        "Floor",
        Vec3::new(0.0, -1.0, 0.0),
        Vec3::new(8.0, 0.1, 8.0),
        tiles,
    );
    {
        let clear = MaterialAsset {
            normal_map: rippled.then(|| ripples("refraction_ripples", 6.0).into()),
            refraction_model: model,
            ior: 1.33,
            thickness: 0.6,
            preserve_specular: false,
            ..glass([1.0; 3], 0.0)
        };
        slab(
            &mut scene,
            "Water",
            Vec3::ZERO,
            Vec3::new(8.0, 0.01, 8.0),
            clear,
        );
    }
    scene
}

#[test]
fn rippled_water_bends_the_floor_and_none_does_not() {
    let cam = Camera::new(Vec3::new(0.0, 3.0, 3.0), -90.0, -50.0);
    let Some(flat) = shot(&pool(RefractionModel::None, false), &cam, "refr_flat") else {
        return;
    };
    let straight = shot(&pool(RefractionModel::None, true), &cam, "refr_none").unwrap();
    let bent = shot(&pool(RefractionModel::Planar, true), &cam, "refr_planar").unwrap();
    let (moved, kept) = (differing(&bent, &flat), differing(&straight, &flat));
    eprintln!("[1523] floor pixels moved: planar {moved}, none {kept}");
    assert!(moved > 200, "the ripples bend the checker: {moved}");
    assert!(kept < 5, "no refraction, no bending: {kept}");
}

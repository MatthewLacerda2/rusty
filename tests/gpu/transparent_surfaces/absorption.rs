//! Absorption (#1523, HDRP's *Transmittance Color* at an *Absorption Distance*):
//! the same water over a deep floor reads darker and bluer than over a shallow one,
//! while clear water (white transmittance) leaves both alike.

use glam::Vec3;
use rusty::components::MaterialAsset;
use rusty::scene::Camera;

use super::{glass, mean_in, shot, slab, stage, SIZE};

/// A white floor `depth` under a water surface whose light keeps `transmittance`
/// after one metre.
fn pool(depth: f32, transmittance: [f32; 3]) -> rusty::scene::Scene {
    let mut scene = stage(-1.4);
    let floor = MaterialAsset {
        base_color: [0.9; 3],
        roughness: 1.0,
        ..MaterialAsset::default()
    };
    slab(
        &mut scene,
        "Floor",
        Vec3::new(0.0, -depth, 0.0),
        Vec3::new(60.0, 0.1, 60.0),
        floor,
    );
    let water = MaterialAsset {
        transmittance_color: transmittance,
        absorption_distance: 1.0,
        preserve_specular: false,
        roughness: 1.0,
        ..glass([1.0; 3], 0.0)
    };
    slab(
        &mut scene,
        "Water",
        Vec3::ZERO,
        Vec3::new(60.0, 0.01, 60.0),
        water,
    );
    scene
}

/// The mean brightness of the middle of `scene` seen from above.
fn middle(scene: &rusty::scene::Scene, name: &str) -> Option<f64> {
    let cam = Camera::new(Vec3::new(0.0, 2.0, 0.0), -90.0, -89.0);
    let img = shot(scene, &cam, name)?;
    Some(mean_in(
        &img,
        (SIZE / 4, 3 * SIZE / 4),
        (SIZE / 4, 3 * SIZE / 4),
    ))
}

#[test]
fn deep_water_absorbs_more_than_shallow_water() {
    let teal = [0.3, 0.6, 0.7];
    let Some(shallow) = middle(&pool(0.2, teal), "abs_shallow") else {
        return;
    };
    let deep = middle(&pool(4.0, teal), "abs_deep").unwrap();
    let clear_shallow = middle(&pool(0.2, [1.0; 3]), "abs_clear_shallow").unwrap();
    let clear_deep = middle(&pool(4.0, [1.0; 3]), "abs_clear_deep").unwrap();
    eprintln!(
        "[1523] floor through water: shallow {shallow:.1}, deep {deep:.1}; \
         clear {clear_shallow:.1} / {clear_deep:.1}"
    );
    assert!(
        deep < shallow * 0.5,
        "the deep end darkens: {deep:.1} vs {shallow:.1}"
    );
    let alike = (clear_deep - clear_shallow).abs() < clear_shallow * 0.15;
    assert!(
        alike,
        "clear water absorbs nothing: {clear_deep:.1} vs {clear_shallow:.1}"
    );
}

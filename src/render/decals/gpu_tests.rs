//! Surface decals are lit (#638): a decal changes the floor's material before the
//! floor is lit, so it is dark where its surface is dark, bright and coloured under
//! a lamp, and a wet decal catches the lamp's highlight. Skips with no adapter.

use glam::Vec3;

use super::gpu_fixture::{centre, grey, lamp, red, stage, stamp};
use crate::components::{DecalBlend, MaterialAsset};
use crate::render::test_gpu::headless_or_skip;
use crate::scene::decal::DecalSpec;

#[test]
fn gpu_a_decal_is_dark_unlit_and_coloured_under_a_lamp() {
    let Some(mut renderer) = headless_or_skip(96, 96) else {
        return;
    };
    let marked = |lit: bool| {
        let mut scene = stage(grey(0.8), 0.0);
        stamp(&mut scene, Vec3::Y, red());
        if lit {
            lamp(&mut scene, 2.0);
        }
        scene
    };
    let (unlit, counters) = centre(&mut renderer, &marked(false), false);
    assert_eq!(counters.decals_visible, 1, "{counters:?}");
    // Before #638 the stamp was drawn at full brightness over the unlit floor.
    assert!(
        unlit.iter().sum::<i32>() < 30,
        "an unlit decal glows: {unlit:?}"
    );
    let (shown, _) = centre(&mut renderer, &marked(true), false);
    assert!(
        shown[0] > 80,
        "the lamp does not light the decal: {shown:?}"
    );
    assert!(shown[0] > shown[1] + 50, "the decal is not red: {shown:?}");
}

#[test]
fn gpu_a_wet_decal_catches_the_lamps_highlight() {
    let Some(mut renderer) = headless_or_skip(96, 96) else {
        return;
    };
    // Changes roughness alone: what "wet" means.
    let wet = MaterialAsset {
        roughness: 0.08,
        decal: DecalBlend {
            albedo: 0.0,
            normal: 0.0,
            metallic: 0.0,
            ..DecalBlend::default()
        },
        ..MaterialAsset::default()
    };
    let floor = |stamped: bool| {
        let mut scene = stage(grey(0.1), 0.0);
        lamp(&mut scene, 3.0);
        scene.materials.insert("wet".into(), wet.clone());
        if stamped {
            let material = Some("wet".into());
            let spec = DecalSpec {
                material,
                ..DecalSpec::default()
            };
            stamp(&mut scene, Vec3::Y, spec);
        }
        scene
    };
    let (matte, _) = centre(&mut renderer, &floor(false), false);
    let (shiny, _) = centre(&mut renderer, &floor(true), false);
    let sum = |p: [i32; 3]| p.iter().sum::<i32>();
    assert!(
        sum(shiny) > sum(matte) + 60,
        "no highlight on the wet patch: {shiny:?} vs dry {matte:?}"
    );
}

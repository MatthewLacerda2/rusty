//! SSIL over a directional bake's specular lobe (#1750, #1695): the lobe is the
//! `Baked` lamps' light, which the lightmap already holds, so a glossy floor that
//! shows it gains nothing from SSIL, as the lightmap's own light gains nothing.

use super::baked::{bake_with, off_and_on, BAKED_GAIN};
use crate::components::{LightMode, Ssil};
use crate::render::test_gpu::ssil::{energy, point, renderer, scene, set_ssil, settled, LAMP};
use crate::scene::Scene;

/// Give the fixture's floor `roughness`.
fn floor_roughness(scene: &mut Scene, roughness: f32) {
    scene.materials.get_mut("floor").unwrap().roughness = roughness;
}

#[test]
fn gpu_a_glossy_floor_lit_by_a_baked_lamps_lobe_gains_nothing() {
    let Some(mut r) = renderer() else { return };
    let (mut scene, lamp) = scene(Ssil::default(), LAMP);
    scene
        .world
        .set_light(lamp, Some(point(LAMP, LightMode::Baked)));
    floor_roughness(&mut scene, 0.15);
    assert!(
        bake_with(&mut scene, "lobe", true) > 0,
        "the floor is lightmapped"
    );
    assert!(
        scene.lightmaps.baked_light(lamp),
        "the bake took the lamp in"
    );

    let (off, with) = off_and_on(&mut r, &mut scene);
    // The lobe is there to keep out: the same floor rough shows less light.
    floor_roughness(&mut scene, 1.0);
    set_ssil(&mut scene, Ssil::default());
    let rough = energy(&settled(&mut r, &scene, 4));
    eprintln!("[ssil] glossy baked floor: {off} off, {with} on; {rough} rough");
    assert!(off > rough, "the lobe shows: {off} vs {rough}");
    assert!(with <= off * (1.0 + BAKED_GAIN), "{with} vs {off}");
}

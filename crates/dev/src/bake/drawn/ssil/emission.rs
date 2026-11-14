//! Emission over a lightmap bake (#1750): a static mesh the bake took in emits in
//! it whether or not it receives a lightmap, so SSIL gathers none of its glow; a
//! mesh that isn't static is in no bake, and its glow still bounces.

use glam::Vec3;

use super::baked::{bake, off_and_on, BAKED_GAIN};
use rusty_components::Ssil;
use rusty_render::test_gpu::ssil::{block, floor, on, patch, renderer, set_ssil, settled, FLOOR};
use rusty_scene::Scene;

/// The fixture's floor and, where its red wall stands, a wall glowing red that
/// gets no lightmap (Scale In Lightmap 0), baked. Returns the scene and the wall.
fn glowing_wall_over_a_baked_floor() -> (Scene, u32) {
    let mut scene = floor(Ssil::default());
    let at = (Vec3::new(0.0, 1.5, -6.0), Vec3::new(6.0, 3.0, 0.2));
    let wall = block(&mut scene, "glow", at, [0.0; 3]);
    scene.materials.get_mut("glow").unwrap().emissive = [1.5, 0.0, 0.0];
    scene.world.mesh_mut(wall).unwrap().scale_in_lightmap = 0.0;
    assert!(bake(&mut scene, "emission") > 0, "the floor is lightmapped");
    assert!(
        scene.lightmaps.get(wall).is_none(),
        "the wall holds no lightmap"
    );
    (scene, wall)
}

#[test]
fn gpu_a_static_glow_with_no_lightmap_is_not_gathered_and_a_moving_one_is() {
    let Some(mut r) = renderer() else { return };
    let (mut scene, wall) = glowing_wall_over_a_baked_floor();
    let (off, with) = off_and_on(&mut r, &mut scene);
    eprintln!("[ssil] static glow in the bake: {off} off, {with} on");
    assert!(
        off > 0.0 && with <= off * (1.0 + BAKED_GAIN),
        "its bounce is baked: {with} vs {off}"
    );

    // The same wall, no longer static: a moving prop no bake holds.
    scene.world.set_static(wall, false);
    let red_over_green = |r: &mut _, scene: &mut Scene, ssil| {
        set_ssil(scene, ssil);
        let [red, green, _] = patch(&settled(r, scene, 4), FLOOR);
        red / green
    };
    let off = red_over_green(&mut r, &mut scene, Ssil::default());
    let lit = red_over_green(&mut r, &mut scene, on(1.0));
    eprintln!("[ssil] a moving glow: red/green {off} off, {lit} on");
    assert!(lit >= off * 1.1, "it still bounces");
}

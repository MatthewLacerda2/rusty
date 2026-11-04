//! The `Baked` lamps' list (#1703) is kept: a frame that changed no light, pose or
//! activity rebuilds nothing; moving a lamp, or switching its mode, does.

use glam::Vec3;

use super::fixture::{draw, fog, lamp, renderer, room, view, LAMP_AT};
use crate::components::LightMode;

#[test]
fn gpu_the_baked_lamps_are_rebuilt_only_when_a_light_changes() {
    let Some(mut r) = renderer() else { return };
    let mut scene = room(fog());
    let id = lamp(&mut scene, LAMP_AT, 20.0, LightMode::Baked);
    lamp(
        &mut scene,
        Vec3::new(2.0, 1.0, -3.0),
        5.0,
        LightMode::Realtime,
    );
    let mut v = view(&r);
    let rebuilds = |r: &crate::render::Renderer| r.post_passes.volumetric_fog.baked.rebuilds;
    draw(&mut r, &mut v, &scene);
    let first = rebuilds(&r);
    assert_eq!(
        r.post_passes.volumetric_fog.baked.lamps().len(),
        1,
        "the Baked lamp alone"
    );
    for _ in 0..3 {
        draw(&mut r, &mut v, &scene);
    }
    assert_eq!(rebuilds(&r), first, "nothing changed");
    scene.world.transform_mut(id).unwrap().position = Vec3::new(-1.0, 1.5, -4.0);
    draw(&mut r, &mut v, &scene);
    assert_eq!(rebuilds(&r), first + 1, "a lamp moved");
    let moved = r.post_passes.volumetric_fog.baked.lamps()[0].position;
    assert_eq!(moved, [-1.0, 1.5, -4.0]);
    scene.world.light_mut(id).unwrap().mode = LightMode::Realtime;
    draw(&mut r, &mut v, &scene);
    assert!(
        r.post_passes.volumetric_fog.baked.lamps().is_empty(),
        "now live"
    );
}

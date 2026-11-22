use glam::Vec3;

use super::static_tests::{frame, RES};
use crate::scene::occlusion_fixture::{baked, nearest, ROOM_1};
use crate::{RenderView, Renderer};
use rusty_components::ShadowCasting;
use rusty_lighting::occlusion::test_level::corridors;
use rusty_scene::Scene;

const PATH: [Vec3; 3] = [ROOM_1, Vec3::new(5.0, 1.5, 1.0), Vec3::new(17.0, 1.5, 0.0)];

fn place_lamp(scene: &mut Scene, at: Vec3) {
    let lamp = scene.world.ids_with_light().into_iter();
    let lamp = lamp.filter(|&id| scene.world.name(id).is_some_and(|n| n.as_str() == "Lamp"));
    let lamp = lamp.last().expect("the level's lamp");
    scene.world.transform_mut(lamp).unwrap().position = at;
    let mut light = scene.world.light(lamp).unwrap().clone();
    (light.cast_shadows, light.range, light.intensity) = (true, 14.0, 30.0);
    scene.world.set_light(lamp, Some(light));
}

fn shot(r: &mut Renderer, view: &mut RenderView, scene: &Scene) -> (Vec<u8>, u32) {
    let atlas = frame(r, view, scene);
    (atlas, r.frame_counters.shadow_occlusion_culled)
}

#[test]
fn gpu_a_carried_lamp_culls_behind_walls_and_draws_the_same_atlas() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, crate::OFFSCREEN_FORMAT, RES, RES);
    let (mut culled, mut open) = (baked(), corridors());
    for (k, at) in PATH.into_iter().enumerate() {
        place_lamp(&mut culled, at);
        place_lamp(&mut open, at);
        let (with, n) = shot(&mut r, &mut view, &culled);
        let (without, none) = shot(&mut r, &mut view, &open);
        assert!(n > 0, "step {k} {at}: nothing culled behind the walls");
        assert_eq!(none, 0, "step {k}: no bake culls nothing");
        assert!(
            with == without,
            "step {k} {at}: the PVS moved shadow texels"
        );
    }
    let mut silent = baked();
    let wall = nearest(&silent, "Wall", Vec3::new(10.0, 1.5, 5.0));
    silent.world.mesh_mut(wall).unwrap().cast_shadows = ShadowCasting::Off;
    place_lamp(&mut silent, ROOM_1);
    assert_eq!(shot(&mut r, &mut view, &silent).1, 0);
}

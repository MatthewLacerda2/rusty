use glam::Vec3;

use super::gpu_tests::{drawn, offscreen, shot, H, W};
use rusty_lighting::occlusion::test_level::block;
use rusty_scene::{Camera, Scene};

const LATER: u8 = 3;

fn boxes() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    for i in 0..400u32 {
        let at = Vec3::new(
            (i % 25) as f32 * 0.4 - 5.0,
            (i / 25) as f32 * 0.4 - 3.0,
            0.0,
        );
        let id = block(&mut scene, "Box", at, at + Vec3::splat(0.3));
        let colour = if i < 300 { i % 200 } else { 200 + i % 50 };
        let key = scene.ensure_material_key(id).unwrap();
        let b = colour as f32 / 250.0;
        scene.materials.get_mut(&key).unwrap().base_color = [b, 1.0 - b, 0.5];
        if i >= 300 {
            scene.world.set_layer(id, LATER);
        }
    }
    scene
}

#[test]
fn gpu_more_batches_in_the_same_instance_room_draw_from_the_grown_args() {
    let Some(mut r) = crate::test_gpu::culling::culling_or_skip(W, H) else {
        return;
    };
    let scene = boxes();
    let mut cam = Camera::new(Vec3::new(0.0, 0.5, 8.0), -90.0, 0.0);
    cam.culling_mask = !(1 << LATER);
    let mut view = offscreen(&r);
    shot(&mut r, &mut view, &scene, &cam);
    assert_eq!(view.hiz.cameras[0].drawn_batches, 200, "frame 1's batches");
    let args = view.hiz.cameras[0].final_args().size();
    assert_eq!(args, 8192, "frame 1's args, the final and late sets'");
    cam.culling_mask = u32::MAX;
    let (_, c) = shot(&mut r, &mut view, &scene, &cam);
    assert_eq!(view.hiz.cameras[0].drawn_batches, 250, "frame 2's batches");
    let args = view.hiz.cameras[0].final_args().size();
    assert_eq!(args, 16384, "the args grew, the case this test is for");
    assert_eq!(c.hiz_tested, 400, "{c:?}");
    assert_eq!(drawn(&r, &view).iter().sum::<u32>(), 400, "nothing hidden");
}

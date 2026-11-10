//! A frame whose batch count grows past its args buffer's room while its instance
//! count stays in its own (#2021): the cached draws must follow the grown args
//! buffer, or the 205th indirect draw overruns the old 4096-byte one.

use glam::Vec3;

use super::gpu_tests::{drawn, offscreen, shot, H, W};
use crate::scene::authoring::material;
use crate::scene::lighting::occlusion::test_level::block;
use crate::scene::{Camera, Scene};

/// The layer frame 1's camera leaves out.
const LATER: u8 = 3;

/// 400 boxes in front of the camera, of 250 colours: 300 in the first 200 on
/// the default layer, then 100 in the last 50 on [`LATER`].
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
        let key = material::ensure_material_key(&mut scene, id).unwrap();
        let b = colour as f32 / 250.0;
        material::set_base_color(&mut scene.materials, &key, [b, 1.0 - b, 0.5]);
        if i >= 300 {
            scene.world.set_layer(id, LATER);
        }
    }
    scene
}

#[test]
fn gpu_more_batches_in_the_same_instance_room_draw_from_the_grown_args() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let scene = boxes();
    let mut cam = Camera::new(Vec3::new(0.0, 0.5, 8.0), -90.0, 0.0);
    cam.culling_mask = !(1 << LATER);
    let mut view = offscreen(&r);
    shot(&mut r, &mut view, &scene, &cam);
    assert_eq!(view.hiz.cameras[0].drawn_batches, 200, "frame 1's batches");
    // The same scene and tables: 250 batches outgrow 4096 bytes of args, while
    // 400 instances stay in the 8192 bytes of kept instances 300 took.
    cam.culling_mask = u32::MAX;
    let (_, c) = shot(&mut r, &mut view, &scene, &cam);
    assert_eq!(view.hiz.cameras[0].drawn_batches, 250, "frame 2's batches");
    assert_eq!(c.hiz_tested, 400, "{c:?}");
    assert_eq!(drawn(&r, &view).iter().sum::<u32>(), 400, "nothing hidden");
}

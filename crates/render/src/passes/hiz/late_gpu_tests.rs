//! The late set (#2336): the prepass keeps Hi-Z phase one's depth and draws only
//! what the test found visible that phase one left out. Its depth is the depth a
//! full prepass of the final set leaves, byte for byte, held still and on the
//! frame a box comes out from behind the wall, with a moving material and a
//! clustered mesh in view, with SSAO on (its prepass) and off (the priming one).
//! Where priming runs (not Metal) the forward pass writes no depth, so the view's
//! depth after the frame is the prepass's; on Metal it is the forward pass's own.

use glam::{Quat, Vec3};

use super::cluster_gpu_tests::panel;
use super::gpu_tests::{facing_the_wall, offscreen, shot, walled, words, H, W};
use super::waves_gpu_tests::waves;
use crate::readback::read_texture_bytes;
use crate::{RenderView, Renderer};
use rusty_components::CameraComponent;
use rusty_scene::create::{create_entity, default_post_processing, Primitive};
use rusty_scene::Scene;

/// The wall and the box behind it, a clustered panel and a plane drawn with
/// `water` in front of it, under a volume with AO `ssao`: the scene and the box.
fn scene(water: &str, ssao: bool) -> (Scene, u32) {
    let (mut scene, boxed) = walled();
    panel(
        &mut scene,
        "Panel",
        (Vec3::new(1.5, -1.0, 1.0), 2.5, Quat::IDENTITY),
    );
    let id = create_entity(&mut scene, "Water", Some(Primitive::Plane));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (Vec3::new(-3.0, -1.0, 2.0), Vec3::splat(0.2));
    drop(t);
    let key = scene.ensure_material_key(id).unwrap();
    scene.materials.get_mut(&key).unwrap().shader = Some(water.to_string());
    let volume = create_entity(&mut scene, "Volume", None);
    let mut vc = default_post_processing();
    vc.ssao.active = ssao;
    scene.world.set_post_processing(volume, Some(vc));
    let camera = Some(CameraComponent::default());
    scene.world.set_camera(volume, camera);
    (scene, boxed)
}

/// The instances the base camera's last late set drew, over its batches.
fn late(r: &Renderer, view: &RenderView) -> u32 {
    let cam = &view.hiz.cameras[0];
    let batches = cam.drawn_batches as usize;
    let words = words(r, cam.final_args());
    let late = words.chunks_exact(5).skip(batches).take(batches);
    late.map(|a| a[1]).sum()
}

/// One frame's pixels and the depth the view holds after it.
fn frame(r: &mut Renderer, view: &mut RenderView, scene: &Scene) -> (Vec<u8>, Vec<u8>) {
    let (px, _) = shot(r, view, scene, &facing_the_wall());
    let depth = read_texture_bytes(&r.device, &r.queue, &view.depth_texture, (W, H), 4);
    (px, depth)
}

/// `scene`'s frame with the cull off, in a fresh view, then the cull back on.
fn uncull(r: &mut Renderer, scene: &Scene) -> (Vec<u8>, Vec<u8>) {
    r.quality.gpu_occlusion_culling = false;
    let mut fresh = offscreen(r);
    let out = frame(r, &mut fresh, scene);
    r.quality.gpu_occlusion_culling = true;
    out
}

#[test]
fn gpu_the_late_set_leaves_the_full_prepass_s_depth() {
    let water = waves("late");
    for ssao in [true, false] {
        let Some(mut r) = crate::test_gpu::culling::culling_or_skip(W, H) else {
            return;
        };
        let (mut scene, boxed) = scene(&water.0, ssao);
        let mut view = offscreen(&r);
        // The first frame has no history: phase one draws everything.
        frame(&mut r, &mut view, &scene);
        assert_eq!(
            late(&r, &view),
            0,
            "ssao {ssao}: the first frame's late set"
        );
        let held = frame(&mut r, &mut view, &scene);
        assert_eq!(late(&r, &view), 0, "ssao {ssao}: a held view's late set");
        assert!(
            view.hiz.cameras[0].clusters.ran > 0,
            "the panel is drawn by cluster"
        );
        let off = uncull(&mut r, &scene);
        assert!(held.1 == off.1, "ssao {ssao}: held, the depth moved");
        assert!(held.0 == off.0, "ssao {ssao}: held, the pixels moved");
        // Out in front of the wall: the late set holds it alone.
        scene.world.transform_mut(boxed).unwrap().position += Vec3::new(0.0, -1.0, 8.0);
        let out = frame(&mut r, &mut view, &scene);
        assert_eq!(
            late(&r, &view),
            1,
            "ssao {ssao}: the box out from behind the wall"
        );
        let off = uncull(&mut r, &scene);
        assert!(out.1 == off.1, "ssao {ssao}: out, the depth moved");
        assert!(out.0 == off.0, "ssao {ssao}: out, the pixels moved");
        // The frame after, it is in phase one's set.
        frame(&mut r, &mut view, &scene);
        assert_eq!(late(&r, &view), 0, "ssao {ssao}: the box held in view");
    }
}

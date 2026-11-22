use glam::{Quat, Vec3};

use super::cluster_gpu_tests::panel;
use super::gpu_tests::{facing_the_wall, offscreen, shot, walled, words, H, W};
use super::waves_gpu_tests::waves;
use crate::readback::read_texture_bytes;
use crate::{RenderCounters, RenderView, Renderer};
use rusty_components::CameraComponent;
use rusty_scene::create::{create_entity, default_post_processing, Primitive};
use rusty_scene::Scene;

fn scene(water: &str, (ssao, blur): (bool, bool)) -> (Scene, u32) {
    let (mut scene, boxed) = walled();
    panel(
        &mut scene,
        "Panel",
        (Vec3::new(1.5, -1.0, 1.0), 2.5, Quat::IDENTITY),
    );
    let id = create_entity(&mut scene, "Water", Some(Primitive::Plane));
    let mut t = scene.world.transform_mut(id).unwrap();
    let upright = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    (t.position, t.rotation, t.scale) = (Vec3::new(-3.0, 0.5, 1.0), upright, Vec3::splat(0.2));
    drop(t);
    let key = scene.ensure_material_key(id).unwrap();
    scene.materials.get_mut(&key).unwrap().shader = Some(water.to_string());
    let volume = create_entity(&mut scene, "Volume", None);
    let mut vc = default_post_processing();
    vc.ssao.active = ssao;
    scene.world.set_post_processing(volume, Some(vc));
    let camera = Some(CameraComponent {
        motion_blur_active: blur,
        ..CameraComponent::default()
    });
    scene.world.set_camera(volume, camera);
    (scene, boxed)
}

fn late(r: &Renderer, view: &RenderView) -> u32 {
    let cam = &view.hiz.cameras[0];
    let batches = cam.drawn_batches as usize;
    let words = words(r, cam.final_args());
    let late = words.chunks_exact(5).skip(batches).take(batches);
    late.map(|a| a[1]).sum()
}

fn frame(r: &mut Renderer, view: &mut RenderView, scene: &Scene, on: bool) -> Frame {
    r.quality.gpu_occlusion_culling = on;
    let (px, counters) = shot(r, view, scene, &facing_the_wall());
    r.quality.gpu_occlusion_culling = true;
    let depth = read_texture_bytes(&r.device, &r.queue, &view.depth_texture, (W, H), 4);
    (px, depth, counters)
}

type Frame = (Vec<u8>, Vec<u8>, RenderCounters);

#[test]
fn gpu_the_late_set_leaves_the_full_prepass_s_depth() {
    let water = waves("late");
    for case in [(true, false), (false, false), (false, true)] {
        let Some(mut r) = crate::test_gpu::culling::culling_or_skip(W, H) else {
            return;
        };
        let (mut scene, boxed) = scene(&water.0, case);
        let (mut view, mut off) = (offscreen(&r), offscreen(&r));
        let mut step = |r: &mut Renderer, scene: &Scene| {
            let on = frame(r, &mut view, scene, true);
            let late = late(r, &view);
            (on, frame(r, &mut off, scene, false), late)
        };
        let (_, _, first) = step(&mut r, &scene);
        assert_eq!(first, 0, "{case:?}: the first frame's late set");
        let (held, uncut, late_held) = step(&mut r, &scene);
        assert_eq!(late_held, 0, "{case:?}: a held view's late set");
        let primes = u32::from(r.depth_priming && r.priming_recommended);
        assert_eq!(held.2.primed_passes, primes, "{case:?}: primed");
        let velocity = held.2.velocity_passes;
        assert!(
            velocity >= u32::from(case.1),
            "{case:?}: blur writes velocity"
        );
        assert!(held.1 == uncut.1, "{case:?}: held, the depth moved");
        assert!(held.0 == uncut.0, "{case:?}: held, the pixels moved");
        scene.world.transform_mut(boxed).unwrap().position += Vec3::new(0.0, -1.0, 8.0);
        let (out, uncut, late_out) = step(&mut r, &scene);
        assert_eq!(late_out, 1, "{case:?}: the box out from behind the wall");
        assert!(out.1 == uncut.1, "{case:?}: out, the depth moved");
        assert!(out.0 == uncut.0, "{case:?}: out, the pixels moved");
        let (_, _, after) = step(&mut r, &scene);
        assert_eq!(after, 0, "{case:?}: the box held in view");
        let ran = view.hiz.cameras[0].clusters.ran;
        assert!(ran > 0, "the panel is drawn by cluster");
    }
}

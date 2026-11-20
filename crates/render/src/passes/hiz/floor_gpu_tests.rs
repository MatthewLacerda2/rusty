//! The cull's instance floor (#2338): a stacked camera drawing a few cubes, a
//! viewmodel's shape, skips the Hi-Z stage and draws as it does with its
//! `use_occlusion_culling` off, through SSAO's prepass, the priming one and a
//! velocity pass, its forward pass still primed, while the base camera under it, drawing more, still culls. A
//! stacked camera whose count crosses the floor, up and back down, draws the
//! same pixels on every frame as with its flag off.

use glam::Vec3;

use super::gpu_tests::{offscreen, shot, H, W};
use super::MIN_INSTANCES;
use crate::{RenderView, Renderer};
use rusty_components::CameraComponent;
use rusty_scene::create::{create_entity, default_post_processing, Primitive};
use rusty_scene::{Camera, ClearFlags, Scene};

/// The stacked camera's layer, and the one neither camera draws.
const GUN: u8 = 8;
const PARKED: u8 = 9;

/// A box of `size` at `at` on `layer`.
fn block(scene: &mut Scene, at: Vec3, size: f32, layer: u8) -> u32 {
    let id = create_entity(scene, "Cube", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (at, Vec3::splat(size));
    drop(t);
    scene.world.set_layer(id, layer);
    id
}

/// Forty boxes on layer 0 for the base camera (motion blur `blur`), a volume
/// with AO `ssao`, and a `DepthOnly` camera stacked over it drawing layer
/// [`GUN`]: three cubes on it and `MIN_INSTANCES` more parked off it. The scene,
/// the stacked camera and the parked cubes.
fn scene((ssao, blur): (bool, bool)) -> (Scene, u32, Vec<u32>) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let base = scene.add_entity("Camera".to_string());
    let lens = CameraComponent {
        motion_blur_active: blur,
        culling_mask: 1,
        ..CameraComponent::default()
    };
    scene.world.set_camera(base, Some(lens));
    let mut volume = default_post_processing();
    volume.ssao.active = ssao;
    scene.world.set_post_processing(base, Some(volume));
    for k in 0..40 {
        let at = Vec3::new((k % 8) as f32 - 3.5, (k / 8) as f32 - 2.0, -8.0);
        block(&mut scene, at, 0.6, 0);
    }
    let gun = scene.add_entity("GunCamera".to_string());
    let lens = CameraComponent {
        render_order: 1,
        clear_flags: ClearFlags::DepthOnly,
        culling_mask: 1 << GUN,
        ..CameraComponent::default()
    };
    scene.world.set_camera(gun, Some(lens));
    for x in [-0.6, 0.0, 0.6] {
        block(&mut scene, Vec3::new(x, -0.4, -2.0), 0.3, GUN);
    }
    let parked = (0..MIN_INSTANCES)
        .map(|k| {
            let at = Vec3::new((k % 8) as f32 * 0.35 - 1.2, (k / 8) as f32 * 0.35, -3.0);
            block(&mut scene, at, 0.2, PARKED)
        })
        .collect();
    (scene, gun, parked)
}

/// One frame with the stacked camera's flag `on` (it is back on after): its
/// pixels, the Hi-Z instances tested, the stack places the view holds and the
/// forward passes primed.
fn frame(
    r: &mut Renderer,
    view: &mut RenderView,
    (scene, gun): (&mut Scene, u32),
    on: bool,
) -> (Vec<u8>, u32, usize, u32) {
    scene.world.camera_mut(gun).unwrap().use_occlusion_culling = on;
    let eye = Camera::new(Vec3::ZERO, -90.0, 0.0);
    let (px, c) = shot(r, view, scene, &eye);
    scene.world.camera_mut(gun).unwrap().use_occlusion_culling = true;
    (px, c.hiz_tested, view.hiz.cameras.len(), c.primed_passes)
}

/// The step's two frames, flag on in `views.0` and off in `views.1`: their
/// pixels equal and primed alike, under the floor or culling as with the flag
/// off (#2338). The on frame's tested count and Hi-Z places, the off one's count.
fn step(
    r: &mut Renderer,
    (view, off): (&mut RenderView, &mut RenderView),
    at: (&mut Scene, u32),
    label: &str,
) -> (u32, usize, u32) {
    let (px, tested, held, primed) = frame(r, view, (&mut *at.0, at.1), true);
    let (px_off, tested_off, _, primed_off) = frame(r, off, at, false);
    // Culling or under the floor, it is primed as the base is; off, not.
    let primes = u32::from(r.depth_priming && r.priming_recommended);
    assert_eq!((primed, primed_off), (2 * primes, primes), "{label}");
    assert!(px == px_off, "{label}: the stacked camera's pixels moved");
    (tested, held, tested_off)
}

#[test]
fn gpu_a_stacked_camera_under_the_floor_draws_as_with_its_flag_off() {
    // SSAO's prepass, the priming one, and the priming one under a velocity pass.
    for case in [(true, false), (false, false), (false, true)] {
        let Some(mut r) = crate::test_gpu::culling::floored_or_skip(W, H) else {
            return;
        };
        let (mut scene, gun, parked) = scene(case);
        let (mut view, mut off) = (offscreen(&r), offscreen(&r));
        // Three cubes, then three and the parked ones, then three again.
        let mut base = 0;
        for (step, more) in [false, false, true, true, false, false]
            .into_iter()
            .enumerate()
        {
            let layer = if more { GUN } else { PARKED };
            for &id in &parked {
                scene.world.set_layer(id, layer);
            }
            let label = format!("{case:?} step {step}");
            let views = (&mut view, &mut off);
            let (tested, held, tested_off) = self::step(&mut r, views, (&mut scene, gun), &label);
            match (step, more) {
                (0, _) => {
                    base = tested;
                    assert!(base >= MIN_INSTANCES, "{label}: the base culls: {base}");
                    assert_eq!(held, 1, "{case:?}: the stacked camera holds no Hi-Z place");
                }
                (_, false) => {
                    assert_eq!(tested, base, "{label}: only the base tested")
                }
                (_, true) => {
                    assert!(tested >= base + MIN_INSTANCES, "{label}: {tested}");
                    assert_eq!(held, 2, "{case:?}: above the floor it culls");
                }
            }
            assert_eq!(tested_off, base, "{label}: the flag off tests none of it");
        }
    }
}

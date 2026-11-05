//! src/render/scene/stack_gpu_tests.rs — a camera stack that draws (#1706): a
//! `DepthOnly` viewmodel camera over a world camera running TAA. Each camera uploads
//! its own instances and the object table's dirty records mid-render, so the gun's
//! upload must not reach the world's draws: the world, a moving prop in it, draws
//! the same pixels with the gun's camera stacked over it as without it.

use glam::Vec3;

use crate::components::CameraComponent;
use crate::render::motion::gpu_fixture::{cube, eye, renderer, scene, view, H, W};
use crate::render::postfx::effects::motion_blur::overlay_gpu_tests::{paint, VIEWMODEL};
use crate::render::{readback, Renderer};
use crate::scene::{ClearFlags, Scene};

/// A blue wall, a red prop at the left and, with `overlay`, a green gun low and
/// right, drawn by a `DepthOnly` camera alone. The world camera runs TAA and skips
/// the gun's layer. Returns the scene and the prop.
fn stack(overlay: bool) -> (Scene, u32) {
    let (mut scene, world) = scene(false);
    let mut lens = scene.world.camera_mut(world).unwrap();
    (lens.taa_active, lens.culling_mask) = (true, !(1 << VIEWMODEL));
    drop(lens);
    let wall = cube(&mut scene, Vec3::new(0.0, 0.0, -9.0));
    scene.world.transform_mut(wall).unwrap().scale = Vec3::new(12.0, 8.0, 1.0);
    paint(&mut scene, wall, "wall", [0.1, 0.1, 0.8]);
    let prop = cube(&mut scene, Vec3::new(-1.5, 0.0, -5.0));
    paint(&mut scene, prop, "prop", [0.8, 0.1, 0.1]);
    if overlay {
        let camera = scene.add_entity("GunCamera".to_string());
        let lens = CameraComponent {
            render_order: 1,
            clear_flags: ClearFlags::DepthOnly,
            culling_mask: 1 << VIEWMODEL,
            ..Default::default()
        };
        scene.world.set_camera(camera, Some(lens));
        let gun = cube(&mut scene, Vec3::new(0.6, -0.3, -1.5));
        scene.world.transform_mut(gun).unwrap().scale = Vec3::splat(0.3);
        paint(&mut scene, gun, "gun", [0.1, 0.8, 0.1]);
        scene.world.set_layer(gun, VIEWMODEL);
        scene.set_parent(gun, Some(camera)).unwrap();
    }
    (scene, prop)
}

/// Six frames with the prop rising each frame (a posed record each frame); the last.
fn run(r: &mut Renderer, overlay: bool) -> Vec<u8> {
    let (mut scene, prop) = stack(overlay);
    let mut v = view(r);
    let mut last = Vec::new();
    for step in 0..6 {
        scene.world.transform_mut(prop).unwrap().position.y = 0.1 * step as f32;
        let out = v.color_target_view().unwrap();
        r.render(&mut v, &scene, &eye(0.0), &out, false);
        let target = v.color_target().unwrap();
        last = readback::read_texture_rgba8(&r.device, &r.queue, target, W, H);
    }
    last
}

/// Pixels of the columns `xs` whose `channel` leads both others by 40.
fn count(frame: &[u8], xs: std::ops::Range<u32>, channel: usize) -> usize {
    let at = |x: u32, y: u32| &frame[((y * W + x) * 4) as usize..][..3];
    let leads = |p: &[u8]| (0..3).all(|c| c == channel || p[channel] > p[c].saturating_add(40));
    xs.flat_map(|x| (0..H).map(move |y| (x, y)))
        .filter(|&(x, y)| leads(at(x, y)))
        .count()
}

#[test]
fn gpu_a_stacked_viewmodel_leaves_the_world_cameras_draws_as_they_were() {
    let Some(mut stacked) = renderer() else {
        return;
    };
    let mut alone = renderer().expect("one ran");
    let (with_gun, without) = (run(&mut stacked, true), run(&mut alone, false));
    let left = 0..W / 2 - 4;
    assert!(
        count(&without, left.clone(), 0) > 10,
        "the prop is on screen"
    );
    assert!(count(&with_gun, W / 2 + 4..W, 1) > 10, "the gun is drawn");
    let row = |f: &[u8], y: u32| f[((y * W) * 4) as usize..][..(left.end * 4) as usize].to_vec();
    let differing = (0..H).filter(|&y| row(&with_gun, y) != row(&without, y));
    assert_eq!(
        differing.count(),
        0,
        "the gun's camera changed the world's draws"
    );
}

//! Baked occlusion culling per camera (#1714): a long wall seen through a door
//! draws the same pixels on and off; an orthographic camera and one whose near
//! plane clips a wall away cull nothing by occlusion; and a static tween
//! re-checks one fingerprint a frame, not one per baked object.

use std::sync::Arc;

use glam::Vec3;

use crate::components::Projection;
use crate::render::{readback, RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::lighting::occlusion::bake_occlusion;
use crate::scene::lighting::occlusion::test_gap::gap;
use crate::scene::lighting::occlusion::test_level::corridors;
use crate::scene::{Camera, Scene};

const W: u32 = 96;
const H: u32 = 64;

fn baked(mut scene: Scene) -> Scene {
    let data = bake_occlusion(&scene, 4)
        .0
        .expect("the level has occluders");
    *scene.occlusion = Some(Arc::new(data));
    scene
}

/// One frame of `scene` through `cam`, culling by occlusion `on`.
fn shot(r: &mut Renderer, scene: &Scene, cam: &Camera, on: bool) -> (Vec<u8>, RenderCounters) {
    let mut cam = cam.clone();
    cam.use_occlusion_culling = on;
    let mut target = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let out = target.color_target_view().unwrap();
    r.render(&mut target, scene, &cam, &out, false);
    let texture = target.color_target().unwrap();
    let px = readback::read_texture_rgba8(&r.device, &r.queue, texture, W, H);
    (px, r.frame_counters)
}

#[test]
fn gpu_a_long_wall_through_a_door_draws_the_same_pixels_on_and_off() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let x = 3.37;
    let scene = baked(gap(40.0, x).0);
    let mut cam = Camera::new(Vec3::new(x, 1.5, 20.0), -90.0, 0.0);
    cam.fov = 20.0; // the door fills the middle of the frame
    let (off, _) = shot(&mut r, &scene, &cam, false);
    let (on, counters) = shot(&mut r, &scene, &cam, true);
    assert!(on == off, "the PVS moved pixels ({counters:?})");
}

#[test]
fn gpu_an_orthographic_or_far_near_camera_culls_nothing_by_occlusion() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let scene = baked(corridors());
    // Down the corridor: a perspective camera with a near plane close in culls.
    let base = Camera::new(Vec3::new(1.0, 1.6, 0.0), 0.0, 0.0);
    assert!(shot(&mut r, &scene, &base, true).1.occlusion_culled > 0);
    // A minimap over the level, inside the grid.
    let mut ortho = Camera::new(Vec3::new(10.0, 2.5, 0.0), 0.0, -89.0);
    ortho.projection = Projection::Orthographic { size: 12.0 };
    // A near plane past the corridor's side wall: the room behind shows.
    let mut far = Camera::new(Vec3::new(5.0, 1.6, 0.0), 90.0, 0.0);
    far.near = 3.0;
    for (name, cam) in [("orthographic", ortho), ("far near plane", far)] {
        let (off, _) = shot(&mut r, &scene, &cam, false);
        let (on, c) = shot(&mut r, &scene, &cam, true);
        assert_eq!(c.occlusion_culled, 0, "{name}: {c:?}");
        assert!(on == off, "{name}: the PVS moved pixels");
    }
}

#[test]
fn gpu_a_static_tween_rechecks_one_object_a_frame() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let mut scene = baked(corridors());
    let cam = Camera::new(Vec3::new(1.0, 1.6, 0.0), 0.0, 0.0);
    shot(&mut r, &scene, &cam, true);
    let binding = |r: &Renderer, s: &Scene| &r.retained.scene(s.id()).unwrap().occlusion;
    let baked_objects = scene.occlusion.as_ref().unwrap().objects.len();
    assert_eq!(
        binding(&r, &scene).rechecked,
        baked_objects,
        "the first bind is whole"
    );
    let door = scene.world.ids_with_mesh()[5];
    for step in 1..=3 {
        scene.world.transform_mut(door).unwrap().position.y += 0.01 * step as f32;
        shot(&mut r, &scene, &cam, true);
        assert_eq!(binding(&r, &scene).rechecked, 1, "frame {step}");
    }
    shot(&mut r, &scene, &cam, true);
    assert_eq!(binding(&r, &scene).rechecked, 0, "nothing moved");
}

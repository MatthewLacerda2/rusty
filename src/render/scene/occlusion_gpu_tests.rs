//! Baked occlusion culling on the GPU (#1714): in the corridors-and-rooms level, at
//! camera positions down the corridor, in doorways and on cell borders, a frame
//! with the PVS on is byte-identical to one with it off, and the PVS culls a stated
//! floor down the corridor. A moved occluder turns it off; a moved prop is drawn.

use std::sync::Arc;

use glam::Vec3;

use crate::render::{readback, RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::lighting::occlusion::{bake_occlusion, test_level::corridors};
use crate::scene::{Camera, Scene};

const W: u32 = 128;
const H: u32 = 80;

/// Down the corridor, on cell borders (x and y even from the grid's origin),
/// in each doorway, and in the rooms: `(eye, yaw, pitch)`.
const VIEWS: [(Vec3, f32, f32); 10] = [
    (Vec3::new(1.0, 1.6, 0.0), 0.0, 0.0),
    (Vec3::new(4.0, 1.8, 0.0), 0.0, 0.0),
    (Vec3::new(5.0, 1.6, 1.0), 90.0, 0.0),
    (Vec3::new(5.0, 1.6, 1.0), -90.0, -5.0),
    (Vec3::new(15.0, 1.6, -1.0), -90.0, 0.0),
    (Vec3::new(2.0, 1.6, 7.0), -60.0, 0.0),
    (Vec3::new(10.0, 1.8, 0.0), 180.0, 0.0),
    (Vec3::new(19.0, 1.6, 0.5), 180.0, 10.0),
    (Vec3::new(8.0, 1.6, -5.0), 150.0, 0.0),
    (Vec3::new(14.0, 1.0, 3.0), 0.0, -10.0),
];

/// The level with its PVS baked.
fn baked() -> Scene {
    let mut scene = corridors();
    let data = bake_occlusion(&scene, 4)
        .0
        .expect("the level has occluders");
    *scene.occlusion = Some(Arc::new(data));
    scene
}

/// One frame of `scene` from `view` on `r`, with occlusion culling `on`.
fn frame(r: &mut Renderer, scene: &Scene, view: (Vec3, f32, f32), on: bool) -> Shot {
    let mut cam = Camera::new(view.0, view.1, view.2);
    (cam.fov, cam.use_occlusion_culling) = (70.0, on);
    shot(r, scene, &cam)
}

type Shot = (Vec<u8>, RenderCounters);

/// One frame of `scene` through `cam`: its pixels and counters.
fn shot(r: &mut Renderer, scene: &Scene, cam: &Camera) -> Shot {
    let mut target = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let out = target.color_target_view().unwrap();
    r.render(&mut target, scene, cam, &out, false);
    let texture = target.color_target().unwrap();
    let px = readback::read_texture_rgba8(&r.device, &r.queue, texture, W, H);
    (px, r.frame_counters)
}

#[test]
fn gpu_the_pvs_never_changes_a_pixel_and_culls_down_the_corridor() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let scene = baked();
    let mut culled = Vec::new();
    for (k, view) in VIEWS.into_iter().enumerate() {
        let (off, all) = frame(&mut r, &scene, view, false);
        let (on, c) = frame(&mut r, &scene, view, true);
        let (again, _) = frame(&mut r, &scene, view, false);
        assert!(
            off == again,
            "view {k}: two frames off differ, so the test proves nothing"
        );
        assert!(on == off, "view {k} {view:?}: the PVS moved pixels ({c:?})");
        assert_eq!(all.occlusion_culled, 0, "off culls nothing");
        assert_eq!(
            c.visible_entities + c.occlusion_culled,
            all.visible_entities,
            "view {k}"
        );
        culled.push(c.occlusion_culled);
    }
    // Down the corridor from its end: the rooms' far props and walls are hidden.
    assert!(culled[0] >= 8, "culled per view: {culled:?}");
}

#[test]
fn gpu_a_moved_occluder_turns_the_bake_off_and_a_moved_prop_is_drawn() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    // A prop with a draw distance (one that culls nothing here) only is occluded.
    let mut scene = corridors();
    let prop = nearest(&scene, Vec3::new(17.0, 0.5, -7.0));
    scene.world.mesh_mut(prop).unwrap().max_draw_distance = 1000.0;
    let data = bake_occlusion(&scene, 4)
        .0
        .expect("the level has occluders");
    *scene.occlusion = Some(Arc::new(data));
    let view = VIEWS[0];
    let (_, before) = frame(&mut r, &scene, view, true);
    assert!(before.occlusion_culled > 0, "{before:?}");
    // That prop moved since the bake: the rest still cull, it never does.
    scene.world.transform_mut(prop).unwrap().position.y += 0.01;
    let (_, moved) = frame(&mut r, &scene, view, true);
    assert!(
        moved.occlusion_culled > 0 && moved.occlusion_culled < before.occlusion_culled,
        "{moved:?}"
    );
    // An occluder moved: anything may show behind it now, so nothing is culled.
    let wall = nearest(&scene, Vec3::new(2.0, 1.5, 1.0));
    scene.world.transform_mut(wall).unwrap().position.y += 0.01;
    let (_, off) = frame(&mut r, &scene, view, true);
    assert_eq!(off.occlusion_culled, 0, "{off:?}");
    // A camera that leaves the walls' layer out sees past them: it culls nothing.
    let mut scene = baked();
    for id in scene.world.ids_with_mesh() {
        if scene.world.name(id).is_some_and(|n| n.as_str() == "Wall") {
            scene.world.set_layer(id, 3);
        }
    }
    let mut cam = Camera::new(view.0, view.1, view.2);
    cam.culling_mask = !(1 << 3);
    let (_, masked) = shot(&mut r, &scene, &cam);
    assert_eq!(masked.occlusion_culled, 0, "{masked:?}");
}

/// The mesh entity whose origin is nearest `at`.
fn nearest(scene: &Scene, at: Vec3) -> u32 {
    let d = |id: u32| {
        scene
            .world
            .local_to_world(id)
            .w_axis
            .truncate()
            .distance(at)
    };
    let ids = scene.world.ids_with_mesh().into_iter();
    ids.min_by(|&a, &b| d(a).total_cmp(&d(b)))
        .expect("the level has meshes")
}

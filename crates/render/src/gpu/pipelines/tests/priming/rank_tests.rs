//! Over a primed depth the unlit solids draw first (#2484): a lit wall tests the
//! prepass's depth and writes none, so its pixels behind an unlit panel are
//! rejected only if the panel already wrote its own. Whichever of the two meshes
//! the renderer interned first, the panel draws ahead of the wall; an unprimed
//! camera keeps the key order. The scene pass asserts (in a debug build) that the
//! order it was batched in is the priming it draws with. Skips with no adapter.

use glam::Vec3;

use crate::draw::batch::{DrawBatch, OpaqueOrder};
use crate::lod::LodSelection;
use crate::{Frustum, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::{CameraComponent, LightComponent};
use rusty_scene::create::{create_entity, default_light, default_post_processing, Primitive};
use rusty_scene::{Camera, Scene};

const RES: u32 = 64;

/// A lit `wall` and, a fifth of a metre in front of it, an unlit `panel` (a
/// light's own mesh), in one depth bucket (`sort::depth_bucket`), under an AO
/// volume that primes the camera.
fn panel_on_a_wall(wall: Primitive, panel: Primitive) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let facing_the_camera = glam::Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    for (name, kind, z) in [("Wall", wall, -6.0), ("Panel", panel, -5.8)] {
        let id = create_entity(&mut scene, name, Some(kind));
        let mut t = scene.world.transform_mut(id).unwrap();
        (t.position, t.scale) = (Vec3::new(0.0, 0.0, z), Vec3::new(2.0, 2.0, 0.2));
        if kind == Primitive::Plane {
            (t.rotation, t.scale) = (facing_the_camera, Vec3::splat(0.2));
        }
        drop(t);
        if name == "Panel" {
            let light = LightComponent {
                intensity: 0.0,
                ..default_light()
            };
            scene.world.set_light(id, Some(light));
        }
    }
    let volume = create_entity(&mut scene, "Volume", None);
    let mut post = default_post_processing();
    post.ssao.active = true;
    scene.world.set_post_processing(volume, Some(post));
    let lens = CameraComponent::default();
    scene.world.set_camera(volume, Some(lens));
    scene
}

/// Whether each opaque draw is lit, in draw order, batched as a camera `primed`
/// or not would batch them, after one frame interned the meshes; with the frame's
/// primed pass count.
fn drawn(r: &mut Renderer, scene: &Scene, primed: bool) -> (Vec<bool>, u32) {
    r.depth_priming = primed;
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    r.render(&mut view, scene, &cam, &out, false);
    let passes = r.frame_counters.primed_passes;
    let order = r.opaque_order(&view, &cam, true);
    assert_eq!(
        order,
        OpaqueOrder::primed(passes > 0),
        "the rule the pass drew by"
    );
    let frustum = Frustum::from_view_proj(cam.build_view_projection(1.0));
    let lod = LodSelection::for_camera(scene, &cam);
    let solids = r.precreate_solid_resources(scene, &cam, (&frustum, &lod), (false, None, order));
    let lit = |b: &DrawBatch| b.key.in_prepass();
    (solids.draws.opaque.iter().map(lit).collect(), passes)
}

#[test]
fn gpu_over_a_primed_depth_the_unlit_panel_draws_before_the_wall_in_both_interning_orders() {
    for (wall, panel) in [
        (Primitive::Box, Primitive::Plane),
        (Primitive::Plane, Primitive::Box),
    ] {
        let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
            return;
        };
        let scene = panel_on_a_wall(wall, panel);
        let (order, passes) = drawn(&mut r, &scene, true);
        // Metal never primes (`pipelines::priming`): there the pass is unprimed.
        if !r.priming_recommended {
            assert_eq!(passes, 0);
            continue;
        }
        assert_eq!(passes, 1, "AO primes the camera");
        assert_eq!(
            order,
            [false, true],
            "{wall:?} wall, {panel:?} panel: unlit first"
        );
    }
}

#[test]
fn gpu_an_unprimed_camera_keeps_the_key_order() {
    // Unprimed, the two share a depth bucket and the interned mesh order decides,
    // so the two interning orders draw the two sequences, as before #2484.
    let mut seen = Vec::new();
    for (wall, panel) in [
        (Primitive::Box, Primitive::Plane),
        (Primitive::Plane, Primitive::Box),
    ] {
        let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
            return;
        };
        let (order, passes) = drawn(&mut r, &panel_on_a_wall(wall, panel), false);
        assert_eq!(passes, 0);
        seen.push(order);
    }
    assert_eq!(
        seen,
        [[true, false], [false, true]],
        "the wall's mesh interned first draws first"
    );
}

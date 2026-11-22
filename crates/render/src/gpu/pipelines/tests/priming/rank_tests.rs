use glam::Vec3;

use crate::draw::batch::{BatchKey, OpaqueOrder};
use crate::lod::LodSelection;
use crate::{Frustum, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::{CameraComponent, LightComponent};
use rusty_scene::create::{create_entity, default_light, default_post_processing, Primitive};
use rusty_scene::{Camera, Scene};

const RES: u32 = 64;

fn panel_on_a_wall(wall: Primitive, panel: Primitive) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    rusty_scene::create::create_live_light(&mut scene, "Sun", Primitive::DirectionalLight);
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
                mode: rusty_components::LightMode::Mixed,
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

fn drawn(r: &mut Renderer, scene: &Scene, primed: bool) -> (Vec<BatchKey>, u32) {
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
    (solids.draws.opaque.iter().map(|b| b.key).collect(), passes)
}

fn lit(keys: &[BatchKey]) -> Vec<bool> {
    keys.iter().map(BatchKey::in_prepass).collect()
}

#[test]
fn gpu_over_a_primed_depth_the_unlit_panel_draws_before_the_wall_in_both_interning_orders() {
    for ((wall, panel), instanced) in [
        (Primitive::Box, Primitive::Plane),
        (Primitive::Plane, Primitive::Box),
    ]
    .into_iter()
    .flat_map(|pair| [(pair, true), (pair, false)])
    {
        let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
            return;
        };
        let scene = panel_on_a_wall(wall, panel);
        r.instancing = instanced;
        let (order, passes) = drawn(&mut r, &scene, true);
        if !r.priming_recommended {
            assert_eq!(passes, 0);
            continue;
        }
        assert_eq!(passes, 1, "AO primes the camera");
        assert_eq!(
            lit(&order),
            [false, true],
            "{wall:?} wall, {panel:?} panel, instanced {instanced}: unlit first"
        );
    }
}

#[test]
fn gpu_an_unprimed_camera_keeps_the_key_order() {
    for (wall, panel) in [
        (Primitive::Box, Primitive::Plane),
        (Primitive::Plane, Primitive::Box),
    ] {
        let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
            return;
        };
        let (order, passes) = drawn(&mut r, &panel_on_a_wall(wall, panel), false);
        assert_eq!(passes, 0);
        let mut lits = lit(&order);
        lits.sort();
        assert_eq!(lits, [false, true], "{wall:?} wall, {panel:?} panel");
        assert!(
            order.is_sorted(),
            "{wall:?} wall, {panel:?} panel: {order:?}"
        );
    }
}

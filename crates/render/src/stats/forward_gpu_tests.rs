//! The forward pass's draws split by kind (#2334): a lit wall behind an unlit light
//! panel counts one lit draw, primed or not as the pass is, and one unlit draw.

use glam::Vec3;

use crate::{RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_scene::create::{create_entity, default_light, Primitive};
use rusty_scene::{Camera, Scene};

const RES: u32 = 32;

/// A sun, a lit wall, and a light's own panel in front of it, which draws unlit.
fn wall_and_panel() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    rusty_scene::create::create_live_light(&mut scene, "Sun", Primitive::DirectionalLight);
    let wall = create_entity(&mut scene, "Wall", Some(Primitive::Box));
    scene.world.transform_mut(wall).unwrap().position = Vec3::new(0.0, 0.0, -4.1);
    let panel = create_entity(&mut scene, "Panel", Some(Primitive::Plane));
    scene.world.transform_mut(panel).unwrap().position = Vec3::new(0.0, 0.0, -4.0);
    scene.world.set_light(panel, Some(default_light()));
    scene
}

fn counters(r: &mut Renderer) -> RenderCounters {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 0.0), -90.0, 0.0);
    r.render(&mut view, &wall_and_panel(), &cam, &out, false);
    r.frame_counters
}

fn split(c: &RenderCounters) -> [u32; 3] {
    [
        c.forward_lit_primed,
        c.forward_lit_unprimed,
        c.forward_unlit,
    ]
}

#[test]
fn gpu_an_unprimed_pass_counts_its_lit_draws_unprimed() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let c = counters(&mut r);
    assert_eq!(c.primed_passes, 0, "no SSAO, SSIL or GPU cull: not primed");
    assert_eq!(split(&c), [0, 1, 1]);
}

#[test]
fn gpu_a_primed_pass_counts_its_lit_draws_primed() {
    let Some(mut r) = crate::test_gpu::culling::culling_or_skip(RES, RES) else {
        return;
    };
    let c = counters(&mut r);
    // Metal never primes (`primed_passes`): its lit draw is unprimed there.
    let want = match c.primed_passes {
        0 => [0, 1, 1],
        _ => [1, 0, 1],
    };
    assert_eq!(split(&c), want, "primed passes: {}", c.primed_passes);
}

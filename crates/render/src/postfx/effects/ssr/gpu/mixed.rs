//! *Mixed* tracing on a mirror floor, the camera looking steeply down at it and a cube above
//! and ahead of it, off screen: the march cannot see the cube, a ray can.

use glam::Vec3;

use super::fixture::{paint, place, rgb, tuned, Surface, H, MIRROR, W};
use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::{PostProcessingComponent, SsrTracing};
use rusty_scene::{Camera, Scene};

pub(super) fn eye() -> Camera {
    Camera::new(Vec3::new(0.0, 1.5, 0.0), -90.0, -70.0)
}

/// Where the floor under the screen's middle reflects to, 5 m up.
const ABOVE: Vec3 = Vec3::new(0.0, 5.0, -2.4);

pub(super) fn room(
    floor: Surface,
    cube_static: bool,
    tune: impl FnOnce(&mut PostProcessingComponent),
) -> Scene {
    let (mut s, _) = tuned(true, floor, MIRROR, |v| {
        v.ssr.tracing = SsrTracing::Mixed;
        tune(v);
    });
    let cube = s.find_entity_by_name("Cube").unwrap();
    place(&mut s, cube, ABOVE, Vec3::new(4.0, 1.0, 4.0));
    paint(&mut s, cube, "cube", ([0.0; 3], 0.0, 1.0));
    s.materials.get_mut("cube").unwrap().emissive = [4.0, 0.0, 0.0];
    s.world.set_static(cube, cube_static);
    let floor = s.find_entity_by_name("Floor").unwrap();
    s.world.set_static(floor, true);
    s
}

pub(super) fn shot(r: &mut Renderer, scene: &Scene) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let out = view.color_target_view().unwrap();
    for _ in 0..2 {
        r.render(&mut view, scene, &eye(), &out, false);
    }
    readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), W, H)
}

/// The share of the frame's middle third (all floor) the red cube is reflected in.
pub(super) fn red_share(frame: &[u8]) -> f32 {
    let (xs, ys) = (W / 3..2 * W / 3, H / 3..2 * H / 3);
    let all: Vec<_> = ys.flat_map(|y| xs.clone().map(move |x| (x, y))).collect();
    let red = all.iter().filter(|&&p| {
        let [r, g, b] = rgb(frame, p);
        r > g + 60 && r > b + 60
    });
    red.count() as f32 / all.len() as f32
}

fn reflected(r: &mut Renderer, scene: Scene) -> f32 {
    let share = red_share(&shot(r, &scene));
    eprintln!("[ssr] mixed: {share:.2} of the floor reflects the cube");
    share
}

#[test]
fn gpu_mixed_reflects_a_static_cube_the_march_cannot_see_and_ray_marching_does_not() {
    let Some(mut r) = super::fixture::renderer() else {
        return;
    };
    let marched = reflected(
        &mut r,
        room(MIRROR, true, |v| v.ssr.tracing = SsrTracing::RayMarching),
    );
    assert!(marched < 0.02, "the march sees no cube: {marched}");
    assert!(!r.holds_ssr_rays(), "RayMarching holds nothing of the rays");
    assert_eq!(r.frame_counters.ssr_ray_texels, 0);
    let mixed = reflected(&mut r, room(MIRROR, true, |_| {}));
    assert!(mixed > 0.5, "a ray reflects the cube: {mixed}");
    let texels = u64::from(W.div_ceil(2) * H.div_ceil(2));
    assert_eq!(
        r.frame_counters.ssr_ray_texels, texels,
        "Medium traces at half"
    );
    reflected(
        &mut r,
        room(MIRROR, true, |v| v.ssr.tracing = SsrTracing::RayMarching),
    );
    assert!(!r.holds_ssr_rays(), "turned off, it frees the rays' state");
    for divisor in [1, 2] {
        let scene = room(MIRROR, true, |v| {
            v.ssr.quality = rusty_core::settings::quality::EffectQuality::Custom;
            v.ssr.trace_resolution_divisor = divisor;
        });
        let share = reflected(&mut r, scene);
        assert!(share > 0.5, "divisor {divisor} reflects it too: {share}");
    }
}

#[test]
fn gpu_mixed_leaves_a_cube_past_its_reach_a_rough_floor_and_a_moving_cube_to_the_probe() {
    let Some(mut r) = super::fixture::renderer() else {
        return;
    };
    let short = reflected(&mut r, room(MIRROR, true, |v| v.ssr.ray_length = 2.0));
    assert!(short < 0.02, "past ray_length: {short}");
    let satin = ([0.9; 3], 1.0, 0.2);
    let rough = reflected(
        &mut r,
        room(satin, true, |v| v.ssr.ray_min_smoothness = 0.9),
    );
    assert!(rough < 0.02, "rougher than ray_min_smoothness: {rough}");
    let moving = reflected(&mut r, room(MIRROR, false, |_| {}));
    assert!(moving < 0.02, "a cube that is not static: {moving}");
}

#[test]
fn gpu_mixed_falls_back_to_ray_marching_when_the_tree_is_refused_and_says_so_once() {
    let Some(mut r) = super::fixture::renderer() else {
        return;
    };
    rusty_test_support::logs::init();
    rusty_test_support::logs::take();
    crate::rays::limit_for_tests(Some(64));
    let scene = room(MIRROR, true, |_| {});
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let out = view.color_target_view().unwrap();
    for _ in 0..3 {
        r.render(&mut view, &scene, &eye(), &out, false);
    }
    let refused = reflected(&mut r, scene);
    crate::rays::limit_for_tests(None);
    let said = rusty_test_support::logs::take();
    let fallbacks = said
        .iter()
        .filter(|m| m.contains("falls back to RayMarching"));
    assert_eq!(fallbacks.count(), 1, "said once: {said:?}");
    assert!(refused < 0.02, "renders as RayMarching: {refused}");
}

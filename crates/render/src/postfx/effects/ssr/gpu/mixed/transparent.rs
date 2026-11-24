//! A translucent floor under *Mixed*: its own march cannot see the cube off screen, its ray can.

use super::super::fixture::{paint, place, MIRROR};
use super::{red_share, room, shot, shot_in, H, W};
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use glam::Vec3;
use rusty_components::{PostProcessingComponent, RenderMode, SsrTracing};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::Scene;

fn pool(receive: bool, tune: impl FnOnce(&mut PostProcessingComponent)) -> Scene {
    let mut s = room(MIRROR, true, tune);
    let floor = s.materials.get_mut("floor").unwrap();
    (floor.render_mode, floor.alpha, floor.receive_ssr) =
        (RenderMode::Transparent, 0.5, Some(receive));
    s
}

fn reflected(r: &mut Renderer, scene: Scene) -> (f32, u64) {
    let share = red_share(&shot(r, &scene));
    let texels = r.frame_counters.ssr_ray_texels;
    eprintln!("[ssr] mixed pool: {share:.2} reflects the cube, {texels} ray texels");
    (share, texels)
}

#[test]
fn gpu_a_transparent_pool_reflects_under_mixed_what_its_march_cannot_see() {
    let Some(mut r) = super::super::fixture::renderer() else {
        return;
    };
    let marched = |v: &mut PostProcessingComponent| v.ssr.tracing = SsrTracing::RayMarching;
    let (march, none) = reflected(&mut r, pool(true, marched));
    assert!(march < 0.02, "the march sees no cube: {march}");
    assert_eq!(none, 0, "RayMarching traces nothing");
    let (mixed, texels) = reflected(&mut r, pool(true, |_| {}));
    assert!(mixed > 0.3, "the pool's ray reflects the cube: {mixed}");
    let half = u64::from(super::W.div_ceil(2) * super::H.div_ceil(2));
    assert_eq!(
        texels,
        2 * half,
        "the opaque rays and the pool's, both at half"
    );
    let (off, opaque_only) = reflected(&mut r, pool(false, |_| {}));
    assert!(off < 0.02, "receive_ssr off: {off}");
    assert_eq!(opaque_only, half, "no pool rays without receive_ssr");
}

#[test]
fn gpu_a_transparent_pool_below_ray_min_smoothness_traces_no_ray() {
    let Some(mut r) = super::super::fixture::renderer() else {
        return;
    };
    let mut scene = pool(true, |v| v.ssr.ray_min_smoothness = 0.9);
    scene.materials.get_mut("floor").unwrap().roughness = 0.2;
    let (rough, texels) = reflected(&mut r, scene);
    assert!(rough < 0.02, "rougher than ray_min_smoothness: {rough}");
    let half = u64::from(super::W.div_ceil(2) * super::H.div_ceil(2));
    assert_eq!(texels, half, "nothing asks for the pool's prepass");
}

#[test]
fn gpu_the_pool_rays_targets_live_only_while_a_translucent_traces() {
    let Some(mut r) = super::super::fixture::renderer() else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    shot_in(&mut r, &mut view, &pool(true, |_| {}));
    assert!(view.post_fx.holds_transparent_rays(), "tracing: held");
    shot_in(&mut r, &mut view, &pool(false, |_| {}));
    assert!(
        !view.post_fx.holds_transparent_rays(),
        "dropped with receive_ssr off"
    );
    let marched = |v: &mut PostProcessingComponent| v.ssr.tracing = SsrTracing::RayMarching;
    shot_in(&mut r, &mut view, &pool(true, |_| {}));
    shot_in(&mut r, &mut view, &pool(true, marched));
    assert!(
        !view.post_fx.holds_transparent_rays(),
        "dropped under RayMarching"
    );
}

/// A second tracing pool under the first: the nearest takes the ray, so the one behind
/// changes nothing whether it traces or not.
#[test]
fn gpu_the_nearest_of_two_tracing_translucents_takes_the_ray() {
    let Some(mut r) = super::super::fixture::renderer() else {
        return;
    };
    let stacked = |receive: bool| {
        let mut s = pool(true, |_| {});
        let under = create_entity(&mut s, "Under", Some(Primitive::Box));
        place(
            &mut s,
            under,
            Vec3::new(0.0, -0.6, -10.0),
            Vec3::new(20.0, 0.1, 30.0),
        );
        paint(&mut s, under, "under", MIRROR);
        s.world.set_static(under, true);
        let m = s.materials.get_mut("under").unwrap();
        (m.render_mode, m.alpha, m.receive_ssr) = (RenderMode::Transparent, 0.5, Some(receive));
        s
    };
    let behind = shot(&mut r, &stacked(true));
    let texels = r.frame_counters.ssr_ray_texels;
    let alone = shot(&mut r, &stacked(false));
    assert!(red_share(&behind) > 0.3, "the top pool reflects the cube");
    let half = u64::from(W.div_ceil(2) * H.div_ceil(2));
    assert_eq!(texels, 2 * half, "one prepass for both pools");
    assert!(behind == alone, "the pool behind takes no ray");
}

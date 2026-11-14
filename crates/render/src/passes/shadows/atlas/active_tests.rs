//! The persistent active atlas on the GPU (#1711): a dynamic caster that enters a
//! cached tile shows on its first frame and leaves no trace when it goes, and a
//! light that blinks off and on re-bakes nothing. Skips with no adapter.

use glam::Vec3;

use super::static_tests::{bake, block, frame, lamp_room, RES};
use crate::{RenderView, OFFSCREEN_FORMAT};

#[test]
fn gpu_a_dynamic_caster_entering_and_leaving_a_cached_tile() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
    let mut scene = lamp_room();
    let away = Vec3::new(40.0, 1.0, 40.0);
    let crate_id = block(&mut scene, away, Vec3::splat(0.4), false);
    let statics = frame(&mut r, &mut view, &scene);
    let still = frame(&mut r, &mut view, &scene);
    assert!(
        still == statics,
        "out of every tile's range: the statics alone"
    );

    scene.world.transform_mut(crate_id).unwrap().position = Vec3::new(-1.0, 1.0, 0.0);
    let entered = frame(&mut r, &mut view, &scene);
    assert_eq!(bake(&r), (7, 0), "a dynamic caster re-bakes no tile");
    assert!(entered != statics, "it shows on its first frame in range");
    r.shadow_renderer.invalidate_static_cache();
    assert!(
        entered == frame(&mut r, &mut view, &scene),
        "as a full redraw"
    );

    scene.world.transform_mut(crate_id).unwrap().position = away;
    let left = frame(&mut r, &mut view, &scene);
    assert!(left == statics, "gone: its tiles hold their statics again");
}

#[test]
fn gpu_a_blinking_light_rebakes_nothing() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
    let mut scene = lamp_room();
    let spot = (scene.world.ids_with_light().into_iter())
        .find(|&id| scene.world.light(id).unwrap().light_type == rusty_scene::LightType::Spotlight)
        .expect("the room's spotlight");
    let lit = frame(&mut r, &mut view, &scene);
    let intensity = scene.world.light(spot).unwrap().intensity;
    scene.world.light_mut(spot).unwrap().intensity = 0.0;
    frame(&mut r, &mut view, &scene);
    assert_eq!(
        r.frame_counters.shadow_atlas_tiles, 6,
        "off: the point light's alone"
    );
    assert_eq!(bake(&r).1, 0, "and nothing re-baked");
    scene.world.light_mut(spot).unwrap().intensity = intensity;
    let back = frame(&mut r, &mut view, &scene);
    assert_eq!(bake(&r), (7, 0), "back on: every tile where it was");
    assert!(back == lit);
}

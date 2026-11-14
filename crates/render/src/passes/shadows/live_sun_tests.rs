//! A frame with no live sun draws no cascade (#1713), and a sun that goes live
//! mid-Play (a `Light.SetMode`) shadows on its first frame: the static bake a Baked
//! frame skipped never reads as cached. Turned Baked and back, the sun shadows as
//! before. Skips with no adapter.

use crate::gpu::pipelines::surface::cut_depth_tests::{ground_at, slab_yard};
use crate::Renderer;
use rusty_components::LightMode;
use rusty_scene::Scene;

/// The ground's brightness under the slab with the sun in `mode`, and the frame's
/// cascade count and shadow draws.
fn under_slab(r: &mut Renderer, scene: &mut Scene, mode: LightMode) -> (f32, usize, u32) {
    let sun = scene.world.find_by_name("Sun").expect("the sun");
    scene.world.light_mut(sun).unwrap().mode = mode;
    let shade = ground_at(r, scene, 0.0);
    let drawn = r.frame_counters.shadow_draws;
    (shade, r.shadow_renderer.cascades.len(), drawn)
}

#[test]
fn gpu_a_sun_going_live_mid_play_shadows_on_its_first_frame() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(96, 96) else {
        return;
    };
    let (mut scene, slab) = slab_yard(None);
    scene.world.set_static(slab, true);

    let (_, cascades, drawn) = under_slab(&mut r, &mut scene, LightMode::Baked);
    assert_eq!(
        (cascades, drawn),
        (0, 0),
        "a Baked sun: no cascade, no draw"
    );
    assert!(
        r.shadow_renderer.static_cache.iter().all(Option::is_none),
        "nothing was baked, so nothing reads as cached"
    );

    let (first, cascades, drawn) = under_slab(&mut r, &mut scene, LightMode::Realtime);
    assert!(
        cascades > 0 && drawn > 0,
        "a live sun fits and bakes its cascades"
    );
    let (steady, ..) = under_slab(&mut r, &mut scene, LightMode::Realtime);
    under_slab(&mut r, &mut scene, LightMode::Baked);
    let (back, ..) = under_slab(&mut r, &mut scene, LightMode::Realtime);

    scene.world.set_active(slab, false);
    let (open, ..) = under_slab(&mut r, &mut scene, LightMode::Realtime);
    eprintln!("[#1713] first {first} steady {steady} back {back} open {open}");
    assert!(
        first + 20.0 < open,
        "the slab shades on the sun's first frame"
    );
    assert!(
        (first - steady).abs() < 1.0,
        "the first frame is the steady one"
    );
    assert!(
        (back - steady).abs() < 1.0,
        "turned off and on, it shades as before"
    );
}

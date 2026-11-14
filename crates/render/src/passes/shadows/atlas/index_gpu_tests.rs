//! The caster index changes no texel (#1874): on the Kolvik-sized bench level
//! (`n = 5`, three shadowed lamps a room, swinging every frame so every tile
//! re-bakes), the atlas and the frame are bit-identical with the index and with
//! every caster tested, while the index tests far fewer. Skips with no adapter.

use glam::Vec3;

use super::pvs_cost_tests::lit;
use super::static_tests::RES;
use super::ATLAS_SIZE;
use crate::{readback, RenderCounters, RenderView, OFFSCREEN_FORMAT};
use rusty_scene::Camera;

/// Three swinging frames of the bench level, the caster index on or off: the
/// last frame's atlas texels and pixels, and the counters summed.
fn frames(index: bool) -> Option<(Vec<u8>, Vec<u8>, RenderCounters)> {
    let mut r = crate::test_gpu::headless_or_skip(RES, RES)?;
    r.shadow_renderer.index_casters = index;
    let (mut scene, lamps) = lit(5, 9.0);
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(11.0, 1.7, 11.0), 45.0, -5.0);
    let mut sum = RenderCounters::default();
    for frame in 0..3 {
        for &(id, at) in &lamps {
            let dx = 0.05 * (frame as f32 * 0.7).sin();
            scene.world.transform_mut(id).unwrap().position = at + Vec3::new(dx, 0.0, 0.0);
        }
        r.render(&mut view, &scene, &cam, &out, false);
        sum.shadow_atlas_rebaked += r.frame_counters.shadow_atlas_rebaked;
        sum.shadow_cull_tested += r.frame_counters.shadow_cull_tested;
        sum.shadow_draws += r.frame_counters.shadow_draws;
    }
    let atlas = &r.shadow_renderer.atlas.texture;
    let texels = readback::read_texture_rgba8(&r.device, &r.queue, atlas, ATLAS_SIZE, ATLAS_SIZE);
    let pixels =
        readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), RES, RES);
    Some((texels, pixels, sum))
}

#[test]
fn gpu_the_caster_index_leaves_the_atlas_bit_identical() {
    let Some((linear, linear_px, all)) = frames(false) else {
        return;
    };
    let (indexed, indexed_px, some) = frames(true).expect("an adapter, as above");
    assert!(
        all.shadow_atlas_rebaked >= 3 * 30,
        "every tile re-bakes: {all:?}"
    );
    let far = 1.0f32.to_le_bytes();
    let drawn = linear.chunks_exact(4).filter(|t| *t != far).count();
    assert!(
        drawn > 1000,
        "the casters drew into the atlas: {drawn} texels"
    );
    let differing = linear
        .chunks(4)
        .zip(indexed.chunks(4))
        .filter(|(a, b)| a != b);
    assert_eq!(differing.count(), 0, "the index moved atlas texels");
    assert_eq!(linear_px, indexed_px, "the index moved pixels");
    assert_eq!(all.shadow_draws, some.shadow_draws);
    assert!(
        some.shadow_cull_tested * 5 < all.shadow_cull_tested,
        "the index should test far fewer: {} of {}",
        some.shadow_cull_tested,
        all.shadow_cull_tested
    );
}

//! *Mixed* across frames: a static moved after a traced frame is traced where it went, and the
//! trace's targets follow the view's size and the trace's divisor.

use glam::Vec3;

use super::super::fixture::{place, H, MIRROR, W};
use super::{red_share, red_share_at, room, shot, shot_in, ABOVE};
use crate::{RenderView, OFFSCREEN_FORMAT};
use rusty_core::settings::quality::EffectQuality;

#[test]
fn gpu_mixed_traces_a_static_moved_after_a_frame_at_its_new_place() {
    let Some(mut r) = super::super::fixture::renderer() else {
        return;
    };
    let mut scene = room(MIRROR, true, |_| {});
    let before = red_share(&shot(&mut r, &scene));
    assert!(before > 0.5, "a ray reflects the cube: {before}");
    let cube = scene.find_entity_by_name("Cube").unwrap();
    // Under the floor, where no reflected ray goes.
    place(
        &mut scene,
        cube,
        ABOVE - 30.0 * Vec3::Y,
        Vec3::new(4.0, 1.0, 4.0),
    );
    let after = red_share(&shot(&mut r, &scene));
    assert!(
        after < 0.02,
        "the next frame traces it where it went: {after}"
    );
}

#[test]
fn gpu_mixed_follows_a_resize_and_a_change_of_its_trace_divisor() {
    let Some(mut r) = super::super::fixture::renderer() else {
        return;
    };
    let mut scene = room(MIRROR, true, |v| {
        v.ssr.quality = EffectQuality::Custom;
        v.ssr.trace_resolution_divisor = 1;
    });
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let full = red_share(&shot_in(&mut r, &mut view, &scene));
    assert!(full > 0.5, "at full resolution: {full}");
    assert_eq!(r.frame_counters.ssr_ray_texels, u64::from(W * H));
    let cam = scene.find_entity_by_name("Camera").unwrap();
    scene
        .world
        .post_processing_mut(cam)
        .unwrap()
        .ssr
        .trace_resolution_divisor = 2;
    let half = red_share(&shot_in(&mut r, &mut view, &scene));
    assert!(half > 0.5, "at half resolution: {half}");
    let texels = u64::from(W.div_ceil(2) * H.div_ceil(2));
    assert_eq!(
        r.frame_counters.ssr_ray_texels, texels,
        "the divisor is followed"
    );
    let (w, h) = (W * 2 / 3, H * 2 / 3);
    view.resize(&r.device, w, h);
    let small = red_share_at(&shot_in(&mut r, &mut view, &scene), (w, h));
    assert!(small > 0.5, "resized: {small}");
    let texels = u64::from(w.div_ceil(2) * h.div_ceil(2));
    assert_eq!(
        r.frame_counters.ssr_ray_texels, texels,
        "the size is followed"
    );
}

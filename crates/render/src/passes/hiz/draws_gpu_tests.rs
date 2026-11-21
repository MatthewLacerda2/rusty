//! What the cull's passes draw against the cull off, in a view where nothing is
//! hidden (#2387): no triangle is drawn twice. The cull adds phase one's depth
//! draw (`hiz_depth_triangles`, a call per depth batch), which stands in for
//! the depth prepass where one runs, a late-set call per depth batch beside
//! it, and a clustered batch's draw per instance (`cluster_draws`); the forward
//! pass draws the triangles it draws with the cull off.

use glam::{Quat, Vec3};

use super::cluster_gpu_tests::panel;
use super::gpu_tests::{offscreen, shot, H, W};
use crate::{RenderCounters, Renderer};
use rusty_components::CameraComponent;
use rusty_lighting::occlusion::test_level::block;
use rusty_scene::create::default_post_processing;
use rusty_scene::{Camera, Scene};

/// A scene with its camera entity, AO on its volume when `ssao`.
pub(super) fn scene(ssao: bool) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let cam = scene.add_entity("Camera".to_string());
    scene
        .world
        .set_camera(cam, Some(CameraComponent::default()));
    let mut volume = default_post_processing();
    volume.ssao.active = ssao;
    scene.world.set_post_processing(cam, Some(volume));
    scene
}

/// Twelve crates, none in front of another: one batch.
pub(super) fn crates(scene: &mut Scene) {
    for k in 0..12 {
        let at = Vec3::new(
            (k % 4) as f32 * 1.5 - 2.25,
            (k / 4) as f32 * 1.2,
            -0.3 * k as f32,
        );
        block(scene, "Crate", at, at + Vec3::splat(0.8));
    }
}

/// Three clustered panels of one mesh side by side: one batch, drawn by cluster.
fn panels(scene: &mut Scene) {
    for x in [-5.0, -1.5, 2.0] {
        panel(scene, "P", (Vec3::new(x, -1.0, -5.0), 3.0, Quat::IDENTITY));
    }
}

/// A frame's pixels and counters.
pub(super) type Shot = (Vec<u8>, RenderCounters);

/// The first `n` frames of `scene` in a new view, the cull `on` or off.
pub(super) fn frames(r: &mut Renderer, scene: &Scene, on: bool, n: usize) -> Vec<Shot> {
    r.quality.gpu_occlusion_culling = on;
    let cam = Camera::new(Vec3::new(0.0, 1.0, 8.0), -90.0, 0.0);
    let mut view = offscreen(r);
    (0..n).map(|_| shot(r, &mut view, scene, &cam)).collect()
}

/// A settled frame: enough that its read backs describe the view held still.
pub(super) fn settled(r: &mut Renderer, scene: &Scene, on: bool) -> Shot {
    frames(r, scene, on, 6).pop().expect("six frames")
}

/// The cull off, then on: the counters of each, asserting the pixels equal.
fn off_and_on(scene: &Scene) -> Option<(RenderCounters, RenderCounters)> {
    let mut r = crate::test_gpu::culling::culling_or_skip(W, H)?;
    let (px_off, off) = settled(&mut r, scene, false);
    let (px_on, on) = settled(&mut r, scene, true);
    assert!(
        px_on == px_off,
        "the cull moved pixels where nothing is hidden"
    );
    assert!(
        on.hiz_tested > 0 && on.hiz_culled == 0,
        "nothing hidden: {on:?}"
    );
    assert_eq!(on.hiz_late, 0, "held still, phase one drew every instance");
    Some((off, on))
}

#[test]
fn gpu_with_nothing_hidden_the_cull_draws_no_triangle_twice() {
    for (ssao, fill) in [
        (false, crates as fn(&mut Scene)),
        (true, crates),
        (false, panels),
    ] {
        let mut s = scene(ssao);
        fill(&mut s);
        let Some((off, on)) = off_and_on(&s) else {
            return;
        };
        let label = format!("ssao {ssao}, clustered {}", on.cluster_tested > 0);
        let depth = on.hiz_depth_triangles;
        assert!(depth > 0, "{label}: phase one drew the view: {on:?}");
        // One depth batch: phase one's call, and the late set's when the cull
        // primes a camera no prepass would (with AO the prepass draws it; on
        // Metal nothing primes, so with neither no late set is drawn).
        let late = if ssao { 0 } else { on.primed_passes };
        let split = on.cluster_draws.saturating_sub(1);
        assert_eq!(on.draw_calls, off.draw_calls + 1 + late + split, "{label}");
        // Phase one stands in for the prepass where one runs, else adds its own.
        let expected = off.triangles + if ssao { 0 } else { depth };
        assert_eq!(on.triangles, expected, "{label}: {off:?} {on:?}");
    }
}

#[test]
fn gpu_a_clustered_batch_is_one_draw_per_instance_and_its_kept_clusters() {
    let mut s = scene(false);
    panels(&mut s);
    let Some((off, on)) = off_and_on(&s) else {
        return;
    };
    assert_eq!(on.cluster_draws, 3, "a draw per panel: {on:?}");
    assert_eq!(off.cluster_draws, 0);
    // Nothing hidden but what faces away: the panels face the camera.
    assert_eq!(on.cluster_culled, 0, "{on:?}");
    assert_eq!(on.triangles - on.hiz_depth_triangles, off.triangles);
}

#[test]
fn gpu_a_wall_s_hidden_box_is_counted_undrawn() {
    let mut s = scene(false);
    block(
        &mut s,
        "Wall",
        Vec3::new(-6.0, -2.0, -0.2),
        Vec3::new(6.0, 5.0, 0.2),
    );
    let lo = Vec3::new(-0.5, 0.0, -5.5);
    block(&mut s, "Box", lo, lo + Vec3::ONE);
    let Some(mut r) = crate::test_gpu::culling::culling_or_skip(W, H) else {
        return;
    };
    let (_, off) = settled(&mut r, &s, false);
    let (_, on) = settled(&mut r, &s, true);
    assert_eq!(on.hiz_culled, 1, "{on:?}");
    // Each pass draws the wall alone: a box is 12 triangles.
    assert_eq!(on.hiz_depth_triangles, off.triangles - 12, "{on:?}");
    assert_eq!(on.triangles, 2 * (off.triangles - 12), "{off:?} {on:?}");
}

//! How many compute passes the `occlusion` stage records (#2327): the select's
//! own, before phase one's depth draw, then one for the pyramid, the test, the
//! compaction and the cluster cull (one dispatch a stage for the frame, #2417),
//! at one batch and at three; on a timed frame (#2401) one each for the pyramid,
//! the test and each of the cluster cull's three stages (#2406). Timed or not,
//! the cluster cull records the same dispatches in the same order.

use glam::{Quat, Vec3};

use super::cluster_gpu_tests::panel;
use super::gpu_tests::{offscreen, shot, H, W};
use crate::timing::GpuPart;
use rusty_lighting::occlusion::test_level::block;
use rusty_scene::{Camera, Scene};

/// The stage's compute passes, at one clustered batch and at three. Measured:
/// lavapipe 2, Metal 2 (CI's macOS leg); before #2327, 4 + R (7 at three).
const PASSES: u32 = 2;

/// [`PASSES`] on a timed frame, each part its own pass (`GpuTimer::splits`).
/// Measured: lavapipe 6 (the split forced, it has no timestamps): the select,
/// the pyramid, the test and the cluster cull's three dispatch kinds (#2406).
const TIMED_PASSES: u32 = 6;

/// A wall half across `n` clustered panels, each its own mesh and so its own
/// clustered batch.
fn panels(n: usize) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let (lo, hi) = (Vec3::new(-9.0, -3.0, -0.2), Vec3::new(0.0, 12.0, 0.2));
    block(&mut scene, "Wall", lo, hi);
    for (i, x) in [-7.0, -2.0, 3.0].into_iter().enumerate().take(n) {
        let at = Vec3::new(x, -2.0, -5.0 - i as f32);
        panel(&mut scene, &format!("Panel_{i}"), (at, 4.0, Quat::IDENTITY));
    }
    scene
}

/// Render `n` panels with the cull on, untimed and `timed`, then off: each
/// settled cull-on frame's counts must show `n` clustered runs in [`PASSES`]
/// (or [`TIMED_PASSES`]) compute passes, both culls at work, and the same pixels
/// as the cull-off frame. Returns (tested, cluster-tested, draws, phase one's
/// triangles, the rest's), timed or not, the draws without the late prepass's,
/// which only an adapter that primes draws (not Metal, #2336); `None` where no
/// culling adapter is (the test skips).
fn stage_with(n: usize) -> Option<Counts> {
    let (untimed, order) = stage_timed(n, false)?;
    let (timed, timed_order) = stage_timed(n, true)?;
    assert_eq!(timed, untimed, "timing moved the cull");
    // One test, then one scan, then one copy, the same workgroups timed or
    // not: the untimed path runs what the bench times.
    let steps: Vec<_> = order.iter().map(|d| d.0).collect();
    assert_eq!(steps, super::stage::CLUSTER_STEPS, "one dispatch a stage");
    assert_eq!(order, timed_order, "untimed, timed");
    Some(untimed)
}

/// The settled frame's counts, and its cluster cull's dispatches and workgroups.
type Staged = (Counts, Vec<(GpuPart, (u32, u32))>);

type Counts = (u32, u32, u32, u64, u64);

/// A box's triangles, and a panel's (`cluster_gpu_tests::panel`).
const WALL: u64 = 12;
const PANEL: u64 = 8192;

fn stage_timed(n: usize, timed: bool) -> Option<Staged> {
    let mut r = crate::test_gpu::culling::culling_or_skip(W, H)?;
    r.gpu_timer.split_override.set(Some(timed));
    r.time_stages(true);
    let scene = panels(n);
    let cam = Camera::new(Vec3::new(0.0, 1.0, 8.0), -90.0, 0.0);
    let mut view = offscreen(&r);
    // A few frames, so the history and the counters' read back settle.
    let shots: Vec<_> = (0..9)
        .map(|_| shot(&mut r, &mut view, &scene, &cam))
        .collect();
    let (on, c) = shots.last().expect("nine frames");
    let clusters = &view.hiz.cameras[0].clusters;
    assert_eq!(
        clusters.runs_ran as usize, n,
        "each panel its own clustered run: {c:?}"
    );
    let order = clusters.dispatched.borrow().clone();
    let passes = if timed { TIMED_PASSES } else { PASSES };
    assert_eq!(
        c.ops.compute_passes, passes,
        "{n} runs, timed {timed}: {c:?}"
    );
    assert!(c.hiz_culled > 0 && c.cluster_culled > 0, "{c:?}");
    assert_eq!(occlusion_laps(&r), PARTS, "the stage's CPU parts (#2401)");
    r.quality.gpu_occlusion_culling = false;
    let mut fresh = offscreen(&r);
    let (off, _) = shot(&mut r, &mut fresh, &scene, &cam);
    assert!(
        occlusion_laps(&r).is_empty(),
        "no part laps with the cull off"
    );
    assert!(*on == off, "the cull moved pixels");
    // A primed pass's late prepass draws each batch (all lit here) once more.
    let late = c.primed_passes * view.hiz.cameras[0].drawn_batches;
    Some((
        (
            c.hiz_tested,
            c.cluster_tested,
            c.draw_calls - late,
            c.hiz_depth_triangles,
            c.triangles - c.hiz_depth_triangles,
        ),
        order,
    ))
}

/// The stage's CPU laps (`render.occlusion.<part>`), in the order first lapped.
const PARTS: [&str; 7] = [
    "occlusion.hold",
    "occlusion.test",
    "occlusion.clusters",
    "occlusion.select",
    "occlusion.phase_one",
    "occlusion.pyramid",
    "occlusion.record",
];

fn occlusion_laps(r: &crate::Renderer) -> Vec<&'static str> {
    let laps = r.frame_stages().iter().map(|(s, _)| *s);
    laps.filter(|s| s.starts_with("occlusion.")).collect()
}

// Pinned: what every adapter decides alike (the passes, the calls, the wall's
// triangles, the hidden panel's none). Bounded: the kept clusters of a panel
// the wall half hides, the GPU's own texel answer.
#[test]
fn gpu_the_stage_records_two_compute_passes_with_one_clustered_batch() {
    // Measured: lavapipe and Metal (CI's macOS leg) 2/86/4/12/12.
    if let Some(decided) = stage_with(1) {
        assert_eq!(decided, (2, 86, 4, WALL, WALL));
    }
}

#[test]
fn gpu_the_stage_records_two_compute_passes_with_three_clustered_batches() {
    // Measured: lavapipe 4/258/8/16396/12946; Metal the same four, the rest bounded.
    if let Some((tested, clusters, draws, depth, rest)) = stage_with(3) {
        assert_eq!((tested, clusters, draws), (4, 258, 8));
        assert_eq!(depth, WALL + 2 * PANEL, "the wall and the visible panels");
        let (lo, hi) = (WALL + PANEL, WALL + 2 * PANEL);
        assert!((lo..=hi).contains(&rest), "{rest} kept, not in {lo}..={hi}");
    }
}

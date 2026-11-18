//! SSR's GPU calls on a still frame (#2329, `gpu::ops`): the copy, the trace, one
//! render pass per hit-pyramid level below the first (into both pyramids at
//! once; none at 8 px or less, where they are one level) and the resolve, and no
//! bind group made once its targets exist. Pinned against the
//! same scene with SSR off, so what SSR alone adds shows.

use super::fixture::{eye, renderer, scene, H, MIRROR, RED, W};
use crate::gpu::ops::OpCounts;
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};

/// What SSR adds to a still frame at 96×72: the trace and the resolve, the hit
/// pyramids' four levels below the first (one pass each, into both), its own
/// submit and its params write. Before #2329, 20 render passes (each pyramid's six
/// levels below the first) and two bind groups; #2329 built the pyramids in one
/// compute pass (2/1/1/0/1), which #2340 measures against these.
/// Measured: lavapipe 6/0/1/0/1.
const SSR: OpCounts = OpCounts {
    render_passes: 6,
    compute_passes: 0,
    submits: 1,
    bind_groups_created: 0,
    buffer_writes: 1,
};

/// The fourth still frame's counts with SSR `on`.
fn still(r: &mut Renderer, on: bool) -> OpCounts {
    let (s, _) = scene(on, MIRROR, RED);
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let out = view.color_target_view().unwrap();
    for _ in 0..4 {
        r.render(&mut view, &s, &eye(), &out, false);
    }
    r.frame_counters.ops
}

fn minus(a: OpCounts, b: OpCounts) -> OpCounts {
    OpCounts {
        render_passes: a.render_passes - b.render_passes,
        compute_passes: a.compute_passes - b.compute_passes,
        submits: a.submits - b.submits,
        bind_groups_created: a.bind_groups_created - b.bind_groups_created,
        buffer_writes: a.buffer_writes - b.buffer_writes,
    }
}

#[test]
fn gpu_a_still_ssr_frame_adds_a_render_pass_per_pyramid_level_and_no_compute_pass() {
    let Some(mut r) = renderer() else { return };
    let (on, off) = (still(&mut r, true), still(&mut r, false));
    eprintln!("[ssr] still frame: on {on:?}, off {off:?}");
    assert_eq!(minus(on, off), SSR, "on {on:#?}, off {off:#?}");
}

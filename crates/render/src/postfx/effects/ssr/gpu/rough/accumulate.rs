use super::bottles::{eye, shelf, steps, H, W};
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::SsrAlgorithm::{Approximation, PbrAccumulation};
use rusty_scene::Scene;

/// Measured on lavapipe: SSR off 16.5, `Approximation` 35.0, `PBRAccumulation` 12.5.
const STILL: f32 = 20.0;
/// After TAA, measured on lavapipe: 2.7, 9.6 and 1.5.
const TAA_STILL: f32 = 4.0;

#[test]
fn gpu_a_rough_reflection_of_thin_bottles_holds_still_under_pbr_accumulation() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let off = steps(&mut r, &shelf(false, Approximation));
    let now = steps(&mut r, &shelf(true, Approximation));
    let kept = steps(&mut r, &shelf(true, PbrAccumulation));
    eprintln!("[ssr] the shelf's largest tile step, resolved / after TAA: off {off:.1?}, Approximation {now:.1?}, PBRAccumulation {kept:.1?}");
    assert!(
        now.0 > 1.5 * off.0 && now.1 > 2.0 * off.1,
        "the case reproduces: Approximation's bottles blink ({now:.1?}, SSR off {off:.1?})"
    );
    assert!(
        kept.0 <= STILL,
        "accumulated, a still reflection blinks: a tile steps {:.1}",
        kept.0
    );
    assert!(kept.1 <= TAA_STILL, "after TAA a tile steps {:.1}", kept.1);
}

#[test]
fn gpu_a_view_keeps_ssr_history_only_while_it_accumulates() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let mut held = |r: &mut Renderer, s: Scene| {
        let target = v.color_target_view().unwrap();
        r.render(&mut v, &s, &eye(), &target, false);
        v.post_fx.ssr_history()
    };
    assert_eq!(
        held(&mut r, shelf(true, Approximation)),
        None,
        "Approximation keeps none"
    );
    let size = Some((W, H));
    assert_eq!(
        held(&mut r, shelf(true, PbrAccumulation)),
        size,
        "kept at the view's size"
    );
    assert_eq!(
        held(&mut r, shelf(false, PbrAccumulation)),
        None,
        "SSR off drops it"
    );
    assert_eq!(held(&mut r, shelf(true, PbrAccumulation)), size);
    assert_eq!(
        held(&mut r, shelf(true, Approximation)),
        None,
        "switched back, dropped"
    );
}

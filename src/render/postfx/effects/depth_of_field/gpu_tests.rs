//! Depth of field on lavapipe (#1445): focus on the near cube and the far one blurs,
//! focus on the far one and the near one does, on the half-resolution and the
//! full-resolution paths; Gaussian blurs only the background; mode `Off` is
//! byte-identical to no depth of field and holds no targets; no quality preset turns
//! it off (#1522).

use super::gpu_fixture::{bokeh, change_around, frame, renderer, scene, shot, view, FAR, NEAR};
use crate::components::{DepthOfField, DofMode};
use crate::core::settings::quality::QualityPresets;

/// How far around a cube's centre the tests look, in pixels: the cube and a few
/// pixels of what lies beside it.
const NEAR_REACH: i32 = 12;
const FAR_REACH: i32 = 10;

/// At or under this, a cube is untouched; at or over [`BLURRED`], it blurred.
const SHARP: u8 = 4;
const BLURRED: u8 = 16;

#[test]
fn gpu_the_focus_decides_which_cube_blurs_at_half_and_full_resolution() {
    for tier in [2, 1] {
        let Some(mut r) = renderer(tier) else { return };
        let off = shot(&mut r, &scene(DepthOfField::default()).0);

        let near_focus = shot(&mut r, &scene(bokeh(2.0)).0);
        let (n, f) = (
            change_around(&near_focus, &off, NEAR, NEAR_REACH),
            change_around(&near_focus, &off, FAR, FAR_REACH),
        );
        eprintln!("[dof] 1/{tier} focus near: near changed {n}, far {f}");
        assert!(
            n <= SHARP,
            "1/{tier}: the focused near cube stays sharp ({n})"
        );
        assert!(f >= BLURRED, "1/{tier}: the far cube blurs ({f})");

        let far_focus = shot(&mut r, &scene(bokeh(12.0)).0);
        let (n, f) = (
            change_around(&far_focus, &off, NEAR, NEAR_REACH),
            change_around(&far_focus, &off, FAR, FAR_REACH),
        );
        eprintln!("[dof] 1/{tier} focus far: near changed {n}, far {f}");
        assert!(n >= BLURRED, "1/{tier}: the near cube blurs ({n})");
        assert!(
            f <= SHARP,
            "1/{tier}: the focused far cube stays sharp ({f})"
        );
    }
}

#[test]
fn gpu_gaussian_blurs_the_background_only() {
    let Some(mut r) = renderer(1) else {
        return;
    };
    let off = shot(&mut r, &scene(DepthOfField::default()).0);
    let gaussian = DepthOfField {
        mode: DofMode::Gaussian,
        gaussian_start: 4.0,
        gaussian_end: 8.0,
        gaussian_max_radius: 1.5,
        ..DepthOfField::default()
    };
    let blurred = shot(&mut r, &scene(gaussian).0);
    let (n, f) = (
        change_around(&blurred, &off, NEAR, NEAR_REACH),
        change_around(&blurred, &off, FAR, FAR_REACH),
    );
    eprintln!("[dof] gaussian: near changed {n}, far {f}");
    assert!(n <= SHARP, "nearer than the start stays sharp ({n})");
    assert!(f >= BLURRED, "past the end blurs ({f})");
}

#[test]
fn gpu_mode_off_is_identical_to_no_depth_of_field_and_frees_its_targets() {
    let Some(mut r) = renderer(1) else {
        return;
    };
    let untouched = shot(&mut r, &scene(DepthOfField::default()).0);
    let (mut s, cam) = scene(bokeh(2.0));
    let mut v = view(&r);
    let focused = frame(&mut r, &mut v, &s);
    assert!(v.post_fx.holds_dof(), "the depth of field ran");
    assert_ne!(focused, untouched);

    s.world
        .post_processing_mut(cam)
        .unwrap()
        .depth_of_field
        .mode = DofMode::Off;
    let off = frame(&mut r, &mut v, &s);
    assert!(
        off == untouched,
        "mode Off draws exactly what no depth of field does"
    );
    assert!(!v.post_fx.holds_dof(), "Off holds no targets");
    let velocity = v.post_fx.motion.active_target();
    assert!(velocity.is_none(), "nor writes the velocity target for it");
}

#[test]
fn gpu_the_low_preset_still_runs_depth_of_field() {
    let Some(mut r) = renderer(2) else {
        return;
    };
    r.quality = QualityPresets::default().get("Low").unwrap().values;
    let mut v = view(&r);
    frame(&mut r, &mut v, &scene(bokeh(2.0)).0);
    assert!(v.post_fx.holds_dof(), "no preset turns depth of field off");
}

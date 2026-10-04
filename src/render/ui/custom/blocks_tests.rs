//! Each ui block on lavapipe (#427): a white 32×32 Image over black, drawn through a
//! one-block ui shader, shows the block's signature — asserted by pixels rather
//! than a byte-exact golden. Skips with no adapter.

use super::fixture::{look, scene, shot, Baked, RECT, RES};

/// The rect's centre pixel.
const MID: (u32, u32) = (32, 32);

/// #769: an unresolved shader is visible to the test (`built` is false) and still
/// draws an opaque frame with the graphic in standard white — so an all-zero shot can
/// never come from a missing or half-written bake.
#[test]
fn gpu_an_unresolved_ui_shader_draws_standard_and_is_reported() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let (scene, _, _) = scene(Some("test_ui_never_baked"));
    let shot = shot(&mut renderer, &mut None, &scene);
    assert!(!renderer.ui_renderer.shaders.built("test_ui_never_baked"));
    assert_eq!(shot.px(MID.0, MID.1), [255; 4], "standard white");
    assert_eq!(shot.px(0, 0), [0, 0, 0, 255], "opaque backdrop");
}

#[test]
fn gpu_scanlines_darken_every_other_band() {
    let blocks: &[(&str, &[(&str, f32)])] =
        &[("scanlines", &[("spacing", 4.0), ("strength", 1.0)])];
    let Some(shot) = look("scanlines", blocks, 0.0) else {
        return;
    };
    let column: Vec<u8> = (16..48).map(|y| shot.px(MID.0, y)[0]).collect();
    let (lo, hi) = (column.iter().min().unwrap(), column.iter().max().unwrap());
    assert!(*lo < 60 && *hi > 200, "bands: {column:?}");
}

#[test]
fn gpu_rgb_split_fringes_the_edges_and_keeps_the_middle_white() {
    let blocks: &[(&str, &[(&str, f32)])] = &[("rgb_split", &[("offset", 4.0)])];
    let Some(shot) = look("rgb_split", blocks, 0.0) else {
        return;
    };
    let left = shot.px(RECT[0] as u32 + 1, MID.1);
    let right = shot.px((RECT[0] + RECT[2]) as u32 - 2, MID.1);
    assert!(
        left[0] < 40 && left[1] > 200 && left[2] > 200,
        "cyan left: {left:?}"
    );
    assert!(
        right[0] > 200 && right[1] > 200 && right[2] < 40,
        "yellow right: {right:?}"
    );
    assert!(shot.px(MID.0, MID.1)[..3].iter().all(|&c| c > 240));
}

#[test]
fn gpu_glitch_slices_shift_bands_off_the_rect_edge() {
    let params: &[(&str, f32)] = &[("density", 1.0), ("amount", 8.0)];
    let Some(shot) = look("glitch", &[("glitch_slices", params)], 0.0) else {
        return;
    };
    let gap = (16..48).any(|y| {
        let row = shot.row(y);
        row[..8].iter().chain(&row[24..]).any(|&r| r < 40)
    });
    assert!(gap, "some band slid sideways, leaving black at an edge");
    assert!(shot.px(MID.0, MID.1)[0] > 240, "the middle stays lit");
}

#[test]
fn gpu_noise_flicker_dims_by_at_most_its_strength_and_changes_per_tick() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let baked = Baked::new("flicker", &[("noise_flicker", &[("strength", 0.5)])]);
    let (mut scene, _, _) = scene(Some(&baked.0));
    let mut view = None;
    let reds: Vec<u8> = (0..6)
        .map(|tick| {
            scene.ui_time = tick as f32 / 24.0;
            shot(&mut renderer, &mut view, &scene).px(MID.0, MID.1)[0]
        })
        .collect();
    assert!(reds.iter().all(|&r| r >= 125), "never below half: {reds:?}");
    assert!(reds.iter().any(|&r| r < 235), "some tick dims: {reds:?}");
    assert!(
        reds.windows(2).any(|w| w[0] != w[1]),
        "re-rolled per tick: {reds:?}"
    );
}

#[test]
fn gpu_hologram_recolours_to_its_hue() {
    let params: &[(&str, f32)] = &[("hue", 240.0), ("flicker", 0.0)];
    let Some(shot) = look("holo", &[("hologram", params)], 0.0) else {
        return;
    };
    let [r, g, b, _] = shot.px(MID.0, MID.1);
    assert!(
        b > r.saturating_add(60) && b > g.saturating_add(60),
        "blue: {r} {g} {b}"
    );
}

#[test]
fn gpu_dissolve_eats_part_of_the_graphic() {
    let blocks: &[(&str, &[(&str, f32)])] = &[("dissolve", &[("amount", 0.5)])];
    let Some(shot) = look("dissolve", blocks, 0.0) else {
        return;
    };
    let reds: Vec<u8> = (16..48).flat_map(|y| shot.row(y)).collect();
    let gone = reds.iter().filter(|&&r| r < 20).count();
    let kept = reds.iter().filter(|&&r| r > 230).count();
    assert!(
        gone > 100 && kept > 100,
        "gone {gone}, kept {kept} of {}",
        reds.len()
    );
}

#[test]
fn gpu_wipe_reveals_the_left_half_at_half_progress() {
    let blocks: &[(&str, &[(&str, f32)])] = &[("wipe", &[("progress", 0.5), ("softness", 0.01)])];
    let Some(shot) = look("wipe", blocks, 0.0) else {
        return;
    };
    let row = shot.row(MID.1);
    assert!(row[..14].iter().all(|&r| r > 240), "left shown: {row:?}");
    assert!(row[18..].iter().all(|&r| r < 15), "right hidden: {row:?}");
}

#[test]
fn gpu_radial_wipe_sweeps_clockwise_from_twelve() {
    let blocks: &[(&str, &[(&str, f32)])] = &[("radial_wipe", &[("progress", 0.25)])];
    let Some(shot) = look("radial", blocks, 0.0) else {
        return;
    };
    assert!(shot.px(40, 40)[0] > 240, "top-right quarter shown");
    assert!(shot.px(24, 24)[0] < 15, "bottom-left hidden");
    assert!(
        shot.px(24, 40)[0] < 15,
        "top-left hidden (the sweep is clockwise)"
    );
}

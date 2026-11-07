use glam::Vec2;

use super::*;

fn px(width: u32, height: u32) -> PixelSize {
    PixelSize { width, height }
}

#[test]
fn phases_and_bias_are_fsr2s() {
    let display = px(1920, 1080);
    assert_eq!(phases(display, display), 8, "native");
    assert_eq!(phases(px(1280, 720), display), 18, "Quality, 1.5×");
    assert_eq!(phases(px(960, 540), display), 32, "Performance, 2×");
    assert_eq!(mip_bias(display, display), -1.0);
    assert_eq!(mip_bias(px(960, 540), display), -2.0);
    // A zero size never divides by zero.
    assert!(phases(px(0, 0), display) >= 1);
    assert!(mip_bias(px(0, 0), px(0, 0)).is_finite());
}

#[test]
fn the_jitter_flips_y_and_the_motion_flips_to_last_frame() {
    // TAA's jitter moves the image up by `y` (NDC); FSR2's moves it down by its own.
    assert_eq!(fsr2_jitter(Vec2::new(0.25, 0.125)), Vec2::new(0.25, -0.125));
    // A point moving right and up in NDC came from the left and below in uv.
    let ndc_now_minus_prev = [0.2, 0.2];
    let uv = [
        ndc_now_minus_prev[0] * MOTION_VECTOR_SCALE[0],
        ndc_now_minus_prev[1] * MOTION_VECTOR_SCALE[1],
    ];
    assert!(uv[0] < 0.0 && uv[1] > 0.0, "{uv:?}");
}

#[test]
fn the_uniform_packs_each_input_in_its_slot() {
    let p = Fsr2Params::new(&Frame {
        sizes: [(960, 540), (1920, 1080)],
        jitter: Vec2::new(0.25, 0.5),
        index: 0,
        phases: 32,
        reactive: true,
        exposure: (1.5, true),
        lens: [0.7, 0.4],
    });
    assert_eq!(p.sizes, [960.0, 540.0, 1920.0, 1080.0]);
    assert_eq!(p.jitter, [0.25, -0.5, -0.5, 0.5]);
    assert_eq!(p.frame, [0.0, 32.0, 1.0, 0.0]);
    assert_eq!(p.exposure, [1.5, 1.0, 0.7, 0.4]);
}

#[test]
fn fsr2_runs_where_chosen_unless_off_or_supersampled() {
    use crate::core::settings::upscaler::Upscaler::{Fsr2, Taa};
    use crate::render::postfx::effects::taa::history::resolver;
    let (out, half, big) = (px(1920, 1080), px(960, 540), px(2880, 1620));
    assert_eq!(resolver(true, Fsr2, (half, out)), Some(Fsr2));
    assert_eq!(resolver(true, Fsr2, (out, out)), Some(Fsr2), "native AA");
    assert_eq!(
        resolver(true, Fsr2, (big, out)),
        Some(Taa),
        "FSR2 only upscales"
    );
    assert_eq!(resolver(true, Taa, (half, out)), Some(Taa));
    assert_eq!(
        resolver(false, Fsr2, (half, out)),
        None,
        "TAA off: bilinear"
    );
}

//! The camera's TAA knobs as the resolve reads them (#1847): the tune carries them,
//! each quality level is URP's, and a low Base Blend Factor never lets a moving
//! pixel trust its history more than a still one.

use glam::{Vec2, Vec3};

use super::*;
use rusty_scene::Camera;

fn camera() -> Camera {
    Camera::new(Vec3::ZERO, 0.0, 0.0)
}

fn params(tune: TaaTune) -> TaaParams {
    TaaParams::new(
        ((64, 48), (64, 48)),
        (true, Vec2::ZERO, false),
        tune,
        lenses(&[camera()], 1.0),
    )
}

#[test]
fn the_tune_carries_the_camera_knobs() {
    use rusty_components::{CameraComponent, TaaQuality};
    let cam = CameraComponent {
        taa_base_blend_factor: 0.8,
        taa_variance_clamp_scale: 1.1,
        taa_quality: TaaQuality::VeryHigh,
        ..CameraComponent::default()
    };
    let tune = TaaTune::from_camera(&cam);
    // Frame influence is URP's `1 - base_blend`.
    assert!((tune.frame_influence - 0.2).abs() < 1e-6);
    assert_eq!(tune.variance_clamp_scale, 1.1);
    assert_eq!(tune.levels, [3.0, 2.0, 2.0, 1.0]);
}

#[test]
fn each_quality_level_is_urps() {
    use rusty_components::TaaQuality;
    // URP's `DoTemporalAA(clamp, motion, history, central)` per pass.
    let urp = [
        [0.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 1.0, 0.0],
        [2.0, 2.0, 1.0, 0.0],
        [2.0, 2.0, 2.0, 0.0],
        [3.0, 2.0, 2.0, 1.0],
    ];
    assert_eq!(TaaQuality::ALL.map(levels), urp);
}

#[test]
fn a_low_base_blend_never_lets_motion_ghost_more_than_stillness() {
    use rusty_components::CameraComponent;
    // Base 0.6: a still pixel weighs 0.4, above MOVING_FRAME_INFLUENCE (0.25).
    let cam = CameraComponent {
        taa_base_blend_factor: 0.6,
        ..CameraComponent::default()
    };
    let tune = TaaTune::from_camera(&cam);
    let p = params(tune);
    assert!((p.knobs[0] - 0.4).abs() < 1e-6);
    assert!(
        p.rejection[0] >= p.knobs[0],
        "moving {} < still {}",
        p.rejection[0],
        p.knobs[0]
    );
    // At URP's default the moving weight is the constant.
    assert_eq!(
        params(TaaTune::default()).rejection[0],
        MOVING_FRAME_INFLUENCE
    );
}

//! The projection and render-texture ops (#430), and the lens clamps (#1341).

use super::*;

#[test]
fn target_ops_clamp_keep_settings_and_clear() {
    let mut c = CameraComponent::default();
    set_target_post_fx(&mut c, false); // no target yet: a no-op
    assert!(c.target_texture.is_none());

    set_target_texture(&mut c, " minimap ", 0, 99_999);
    let t = c.target_texture.clone().expect("target set");
    assert_eq!(
        (t.name.as_str(), t.width, t.height),
        ("minimap", 1, MAX_TARGET_SIZE)
    );

    set_target_post_fx(&mut c, false);
    set_target_update_every(&mut c, 0);
    set_target_texture(&mut c, "scope", 256, 256);
    let t = c.target_texture.clone().expect("target kept");
    assert_eq!(t.name, "scope");
    assert!(!t.post_fx, "renaming keeps the post-FX choice");
    assert_eq!(t.update_every, 1, "0 clamps to every frame");

    set_target_texture(&mut c, "", 1, 1);
    assert!(c.target_texture.is_none());
}

#[test]
fn projection_names_round_trip_and_size_stays_positive() {
    let mut c = CameraComponent::default();
    let ortho = parse_projection("ORTHOGRAPHIC", -3.0).expect("known name");
    set_projection(&mut c, ortho);
    assert_eq!(c.projection, Projection::Orthographic { size: 0.01 });
    assert_eq!(projection_name(c.projection), "Orthographic");
    assert_eq!(
        parse_projection("perspective", 5.0),
        Some(Projection::Perspective)
    );
    assert_eq!(parse_projection("fisheye", 5.0), None);
}

#[test]
fn lens_ops_hold_unitys_bounds() {
    let mut c = CameraComponent::default();
    set_fov(&mut c, 0.0);
    assert_eq!(c.fov, MIN_FOV);
    set_fov(&mut c, 500.0);
    assert_eq!(c.fov, MAX_FOV);
    for (fov, bound) in [
        (f32::NAN, MIN_FOV),
        (f32::INFINITY, MAX_FOV),
        (f32::NEG_INFINITY, MIN_FOV),
    ] {
        set_fov(&mut c, fov);
        assert_eq!(c.fov, bound, "a non-finite FOV ({fov}) takes the bound");
    }

    set_clip_planes(&mut c, 0.0, 100.0);
    assert_eq!((c.near, c.far), (MIN_NEAR, 100.0), "near stays above 0");
    set_near(&mut c, 150.0);
    assert_eq!(
        (c.near, c.far),
        (150.0, 150.0 + MIN_CLIP_GAP),
        "far is pushed out"
    );
    set_far(&mut c, 1.0);
    assert_eq!(
        c.far,
        150.0 + MIN_CLIP_GAP,
        "far never drops to the near plane"
    );
    let floor = (MIN_NEAR, MIN_NEAR + MIN_CLIP_GAP);
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        set_clip_planes(&mut c, bad, bad);
        assert_eq!(
            (c.near, c.far),
            floor,
            "non-finite planes ({bad}) take the bound"
        );
    }
}

#[test]
fn far_stays_beyond_a_large_near() {
    let mut c = CameraComponent::default();
    for near in [1e6, f32::MAX] {
        set_clip_planes(&mut c, near, 0.0);
        assert!(
            c.far > c.near && c.far.is_finite(),
            "near {near}: far {} must sit beyond {}",
            c.far,
            c.near
        );
    }
}

//! The projection and render-texture ops (#430).

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

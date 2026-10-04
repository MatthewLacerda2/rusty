//! The rotation fields' drag helper (#333): it shows an angle as it is, except that the
//! Euler decomposition's -0 reads as 0.

/// Run `drag_angle` over `value` for one headless egui pass; return the value it left.
fn shown(value: f32) -> f32 {
    let ctx = egui::Context::default();
    let mut v = value;
    let mut out = ctx.run_ui(Default::default(), |ui| {
        super::drag_angle(ui, &mut v);
    });
    out.textures_delta.clear();
    v
}

#[test]
fn drag_angle_leaves_an_angle_it_shows_unchanged() {
    for angle in [45.0, -90.0, 180.0] {
        assert_eq!(shown(angle), angle);
    }
}

#[test]
fn drag_angle_shows_negative_zero_as_zero() {
    let v = shown(-0.0);
    assert!(v == 0.0 && v.is_sign_positive(), "{v:?}");
}

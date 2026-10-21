//! The rotation fields' drag helper (#333): it shows an angle as it is, except that the
//! Euler decomposition's -0 reads as 0. The scale fields show a mirrored scale
//! without rewriting it (#1142).

/// Run `drag` over `value` for one headless egui pass; return the value it left.
fn shown_by(drag: fn(&mut egui::Ui, &mut f32) -> bool, value: f32) -> f32 {
    let ctx = egui::Context::default();
    let mut v = value;
    let mut out = ctx.run_ui(Default::default(), |ui| {
        drag(ui, &mut v);
    });
    out.textures_delta.clear();
    v
}

fn shown(value: f32) -> f32 {
    shown_by(super::drag_angle, value)
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

/// Selecting a mirrored entity used to clamp its scale to 0.01 in the Inspector.
#[test]
fn drag_scale_leaves_a_scale_it_shows_unchanged() {
    for scale in [-1.5, 0.001, 1.0, 40.0] {
        assert_eq!(shown_by(super::drag_scale, scale), scale);
    }
}

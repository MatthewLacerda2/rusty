//! UI look screenshot (#425): the scene from `ui_shapes_scene` rendered headless,
//! asserted by the colour at known pixels (PNG rows run top-down, hence
//! `row = SIZE - 1 - y`). Covers chamfered and rounded corners, a ring and an arc,
//! a linear gradient, a dashed line, a glow, a shadow and the three non-normal
//! blend modes against a known backdrop.

use glam::Vec3;
use rusty::dev::screenshot::capture;
use rusty::scene::Camera;

use super::ui_shapes_scene::{shapes, SIZE};

#[test]
fn shapes_gradients_effects_and_blends_draw() {
    let path = crate::temp::dir().join("rusty_ui_shapes.png");
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    if !capture(&shapes(), &cam, &path, SIZE, SIZE).expect("capture must not error") {
        eprintln!("[ui] no GPU/software adapter — skipping visual assertion");
        return;
    }
    let img = image::open(&path).expect("png").to_rgb8();
    let check = |x: u32, y: u32, want: [u8; 3], what: &str| {
        let got = img.get_pixel(x, SIZE - 1 - y).0;
        let near = got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 4);
        assert!(near, "{what} at ({x}, {y}): got {got:?}, want {want:?}");
    };
    check(48, 48, [255, 0, 0], "chamfered rect centre");
    check(18, 70, [0, 0, 0], "a chamfered corner is cut");
    check(28, 28, [255, 0, 0], "inside the chamfer's cut line");
    check(83, 40, [255, 255, 0], "the hard shadow beside the rect");
    check(128, 48, [0, 255, 0], "rounded rect centre");
    check(98, 18, [0, 0, 0], "a rounded corner is empty");
    check(208, 48, [0, 0, 0], "the ring's hole");
    check(208, 74, [0, 0, 255], "the ring's band");
    check(
        60,
        140,
        [255, 255, 255],
        "the pie's 12-to-3 o'clock quarter",
    );
    check(36, 140, [0, 0, 0], "outside the arc (top-left)");
    check(60, 116, [0, 0, 0], "outside the arc (bottom-right)");
    check(144, 120, [126, 0, 129], "the gradient's middle");
    check(100, 152, [255, 255, 255], "a dash");
    check(108, 152, [0, 0, 0], "a gap");
    check(224, 128, [255, 255, 255], "the glowing square");
    let glow = |x: u32| img.get_pixel(x, SIZE - 1 - 128).0;
    let (near, far) = (glow(242), glow(248));
    assert!(
        near[0] < 8 && near[1] > far[1] && far[1] > 0,
        "glow fades: {near:?} → {far:?}"
    );
    check(48, 208, [192, 128, 128], "additive adds");
    check(120, 208, [64, 128, 128], "multiply darkens");
    check(192, 208, [191, 128, 128], "screen lightens");
}

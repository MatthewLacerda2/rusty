//! UI render pass screenshot (#418): the HUD from `ui_hud_scene` rendered headless,
//! asserted by the colour at known pixels rather than a byte-exact golden (PNG rows
//! run top-down, hence `row = SIZE - 1 - y`). Covers a 50%-filled bar, a 9-sliced
//! panel, a faded CanvasGroup blending in display space, and a RectMask clip.

use glam::Vec3;
use rusty::dev::screenshot::capture;
use rusty::scene::Camera;

use super::ui_hud_scene::{hud, SIZE};

#[test]
fn hud_draws_fill_slices_fades_and_clips() {
    let path = std::env::temp_dir().join("rusty_ui_hud.png");
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    if !capture(&hud(), &cam, &path, SIZE, SIZE).expect("capture must not error") {
        eprintln!("[ui] no GPU/software adapter — skipping visual assertion");
        return;
    }
    let img = image::open(&path).expect("png").to_rgb8();
    let px = |x: u32, y: u32| img.get_pixel(x, SIZE - 1 - y).0;
    let near = |got: [u8; 3], want: [u8; 3]| got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 3);
    let check = |x, y, want, what: &str| {
        let got = px(x, y);
        assert!(
            near(got, want),
            "{what} at ({x}, {y}): got {got:?}, want {want:?}"
        );
    };
    // The bar fills its left half only.
    check(40, 24, [255, 0, 0], "filled half of the bar");
    check(100, 24, [0, 0, 0], "empty half of the bar");
    // 9-slice: the 8-texel frame stays 8 px wide; stretched (Simple) it would be 28.
    check(132, 64, [0, 255, 0], "slice border");
    check(140, 64, [0, 0, 255], "slice centre just inside the border");
    check(184, 20, [0, 255, 0], "slice bottom border");
    // 50% white over black blends in display space: ~128, not linear-space ~188.
    check(64, 176, [128, 128, 128], "faded group");
    // The child overflows its mask; only the part inside the mask shows.
    check(176, 176, [255, 255, 0], "child inside the mask");
    check(208, 176, [0, 0, 0], "child outside the mask (right)");
    check(176, 208, [0, 0, 0], "child outside the mask (top)");
}

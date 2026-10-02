//! The headless editor capture (#731): the default scene captures, the image isn't
//! blank, and the docked panels are where the editor puts them. A smoke check, not a
//! golden: fonts and anti-aliasing differ across platforms, so no pixel is pinned.

use rusty::dev::editor_capture::{self, EditorCaptureOptions};
use rusty::editor::theme::Theme;

const W: u32 = 1280;
const H: u32 = 720;

/// The share of pixels in the column `x0..x1` (middle half of the height) painted in
/// the editor's panel or card fill, give or take a little for the sRGB round trip.
fn panel_share(img: &image::RgbaImage, x0: u32, x1: u32) -> f32 {
    let theme = Theme::dark();
    let fills = [theme.bg_tier1, theme.bg_tier2];
    let near = |p: [u8; 4], c: egui::Color32| {
        p[0].abs_diff(c.r()) <= 2 && p[1].abs_diff(c.g()) <= 2 && p[2].abs_diff(c.b()) <= 2
    };
    let (mut hits, mut total) = (0u32, 0u32);
    for y in H / 4..H * 3 / 4 {
        for x in x0..x1 {
            let p = img.get_pixel(x, y).0;
            total += 1;
            hits += u32::from(fills.iter().any(|&c| near(p, c)));
        }
    }
    hits as f32 / total as f32
}

#[test]
fn the_default_scene_captures_with_its_panels() {
    let out = crate::temp::dir().join("editor.png");
    let game = editor_capture::default_world();
    let opts = EditorCaptureOptions {
        width: W,
        height: H,
        select: Some("Player".to_string()),
        ..Default::default()
    };
    let written = editor_capture::capture(&game, &out, &opts).expect("capture runs");
    if !written {
        eprintln!("no GPU adapter — skipping the editor capture");
        return;
    }
    let img = image::open(&out).expect("a readable PNG").to_rgba8();
    assert_eq!(img.dimensions(), (W, H));

    // Not blank: a rendered scene plus text has far more than a handful of colours.
    let mut colours: Vec<[u8; 4]> = img.pixels().map(|p| p.0).collect();
    colours.sort_unstable();
    colours.dedup();
    assert!(
        colours.len() > 200,
        "only {} distinct colours",
        colours.len()
    );

    // The hierarchy docks left and the inspector right, both in the panel fill; the
    // viewport between them shows the scene, not a panel.
    let edge = W / 20;
    assert!(
        panel_share(&img, 0, edge) > 0.3,
        "no hierarchy panel at the left"
    );
    assert!(
        panel_share(&img, W - edge, W) > 0.3,
        "no inspector panel at the right"
    );
    assert!(
        panel_share(&img, W / 2 - edge, W / 2 + edge) < 0.1,
        "no scene in the viewport"
    );
}

#[test]
fn an_unknown_selection_is_an_error_not_a_blank_inspector() {
    let game = editor_capture::default_world();
    let opts = EditorCaptureOptions {
        select: Some("NoSuchEntity".to_string()),
        ..Default::default()
    };
    let err = editor_capture::capture(&game, crate::temp::dir().join("x.png"), &opts);
    assert!(err.unwrap_err().contains("NoSuchEntity"));
}

//! The headless editor capture (#731): the default scene captures, the image isn't
//! blank, and the docked panels are where the editor puts them. A smoke check, not a
//! golden: fonts and anti-aliasing differ across platforms, so no pixel is pinned.

use rusty::editor::theme::Theme;
use rusty::shell::editor::capture::{self, EditorCaptureOptions};

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
    let game = capture::default_world();
    let opts = EditorCaptureOptions {
        width: W,
        height: H,
        select: Some("Player".to_string()),
        ..Default::default()
    };
    let written = capture::capture(&game, &out, &opts).expect("capture runs");
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
    let game = capture::default_world();
    let opts = EditorCaptureOptions {
        select: Some("NoSuchEntity".to_string()),
        ..Default::default()
    };
    let err = capture::capture(&game, crate::temp::dir().join("x.png"), &opts);
    assert!(err.unwrap_err().contains("NoSuchEntity"));
}

/// The inspector column's pixels (right fifth, middle half of the height).
fn inspector(img: &image::RgbaImage) -> Vec<[u8; 4]> {
    let (w, h) = img.dimensions();
    let rows = h / 4..h * 3 / 4;
    rows.flat_map(|y| (w - w / 5..w).map(move |x| img.get_pixel(x, y).0))
        .collect()
}

#[test]
fn selecting_a_scene_asset_opens_its_inspector_card() {
    let dir = crate::temp::dir();
    let asset = dir.join("level.scene");
    std::fs::write(&asset, "{}").expect("write the asset");
    let game = capture::default_world();
    let mut host = rusty::dev::capture::CaptureHost::new();
    let mut shot = |name: &str, select_asset: Option<String>| {
        let out = dir.join(name);
        let opts = EditorCaptureOptions {
            width: W,
            height: H,
            select_asset,
            ..Default::default()
        };
        let written = capture::capture_into(&mut host, &game, &out, &opts).expect("captures");
        written.then(|| image::open(&out).expect("a readable PNG").to_rgba8())
    };
    let Some(settings) = shot("settings.png", None) else {
        eprintln!("no GPU adapter — skipping the editor capture");
        return;
    };
    let card = shot("card.png", Some(asset.display().to_string())).expect("same adapter");
    // Nothing selected shows the scene settings; the asset swaps in its card.
    assert_ne!(
        inspector(&settings),
        inspector(&card),
        "inspector unchanged"
    );
}

#[test]
fn a_missing_asset_or_a_double_selection_is_an_error() {
    let game = capture::default_world();
    let out = crate::temp::dir().join("x.png");
    let missing = EditorCaptureOptions {
        select_asset: Some("no/such.scene".to_string()),
        ..Default::default()
    };
    let err = capture::capture(&game, &out, &missing).unwrap_err();
    assert!(err.contains("no/such.scene"), "{err}");
    let both = EditorCaptureOptions {
        select: Some("Player".to_string()),
        ..missing
    };
    let err = capture::capture(&game, &out, &both).unwrap_err();
    assert!(err.contains("both"), "{err}");
}

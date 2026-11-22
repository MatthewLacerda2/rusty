//! The editor captured with the Game view at a size of its own: the image
//! letterboxed in the panel, and the UI's screen (`UI.GetScreenSize`) that size, on
//! the Game tab and behind the Scene tab alike.

use rusty::app::ProjectSource;
use rusty::core::application::{GameViewSetting, GameViewSize};
use rusty::editor::ViewportTab;
use rusty::shell::editor::capture::{self, EditorCaptureOptions};

fn sized(tab: ViewportTab, size: GameViewSize) -> EditorCaptureOptions {
    EditorCaptureOptions {
        width: 600,
        height: 900,
        tab,
        game_view: GameViewSetting {
            size,
            low_resolution: false,
        },
        ..Default::default()
    }
}

#[test]
fn a_fixed_size_letterboxes_and_sizes_the_screen() {
    let game = capture::default_world(ProjectSource::defaults());
    let out = crate::temp::dir().join("fixed.png");
    let opts = sized(ViewportTab::Game, GameViewSize::Fixed(320, 180));
    if !capture::capture(&game, &out, &opts).expect("captures") {
        return; // no adapter: the skip contract
    }
    assert_eq!(game.resources.screen.borrow().game_view(), Some((320, 180)));
    let img = image::open(&out).unwrap().to_rgba8();
    // The viewport sits between the hierarchy and the inspector; sample its middle.
    let black = |y: u32| (250..350).all(|x| img.get_pixel(x, y).0[..3] == [0, 0, 0]);
    assert!(black(150), "the bar above the letterboxed view is black");
    assert!(!black(450), "the view itself is drawn");
}

#[test]
fn the_scene_tab_keeps_the_game_views_screen() {
    let game = capture::default_world(ProjectSource::defaults());
    let out = crate::temp::dir().join("scene.png");
    let opts = sized(ViewportTab::Scene, GameViewSize::Fixed(320, 180));
    if !capture::capture(&game, &out, &opts).expect("captures") {
        return;
    }
    assert_eq!(game.resources.screen.borrow().game_view(), Some((320, 180)));
    let opts = sized(ViewportTab::Scene, GameViewSize::Free);
    capture::capture(&game, &out, &opts).expect("captures");
    let (w, h) = game.resources.screen.borrow().game_view().expect("set");
    assert!(h > w, "the tall panel's own pixels: {w}x{h}");
}

use super::*;

fn setting(size: GameViewSize, low_resolution: bool) -> GameViewSetting {
    GameViewSetting {
        size,
        low_resolution,
    }
}

/// `Free` is the panel's own size: no size of its own, so it draws as it always did.
#[test]
fn free_is_the_panel() {
    let free = GameViewSetting::default();
    assert_eq!(free.size, GameViewSize::Free);
    assert_eq!(free.resolve((2940, 1448), 2.0), None);
    assert_eq!(free.resolve((1, 1), 1.0), None);
}

/// An aspect is the largest rectangle of that ratio in the panel: a tall panel
/// letterboxes 16:9 top and bottom, a wide one pillarboxes 4:3.
#[test]
fn aspect_fits_the_panel() {
    let wide = setting(GameViewSize::Aspect(16, 9), false);
    assert_eq!(wide.resolve((800, 1000), 1.0), Some((800, 450)));
    let square = setting(GameViewSize::Aspect(4, 3), false);
    assert_eq!(square.resolve((2000, 600), 1.0), Some((800, 600)));
    // A degenerate panel still gives a 1-pixel side, never a 0.
    assert_eq!(wide.resolve((0, 0), 1.0), Some((1, 1)));
}

/// A fixed size is exact whatever the panel and the scale, "1x on HiDPI" included.
#[test]
fn fixed_is_exact() {
    for low in [false, true] {
        let fixed = setting(GameViewSize::Fixed(1920, 1080), low);
        assert_eq!(fixed.resolve((640, 2000), 2.0), Some((1920, 1080)));
    }
}

/// "1x on HiDPI" sizes `Free` and an aspect by the panel's points; on a 1x panel
/// it changes nothing.
#[test]
fn low_resolution_uses_points() {
    let free = setting(GameViewSize::Free, true);
    assert_eq!(free.resolve((2940, 1448), 2.0), Some((1470, 724)));
    assert_eq!(
        free.resolve((1813, 1054), 1.0),
        None,
        "a 1x panel is its own"
    );
    let aspect = setting(GameViewSize::Aspect(16, 9), true);
    assert_eq!(aspect.resolve((2000, 2000), 2.0), Some((1000, 563)));
}

/// Both sides from 1 to the texture limit; `Free` always.
#[test]
fn validity_bounds_both_sides() {
    assert!(GameViewSize::Free.is_valid());
    assert!(GameViewSize::Fixed(1, MAX_GAME_VIEW_SIDE).is_valid());
    assert!(!GameViewSize::Fixed(0, 10).is_valid());
    assert!(!GameViewSize::Aspect(16, MAX_GAME_VIEW_SIDE + 1).is_valid());
}

/// A preset shows its label; a custom size its numbers.
#[test]
fn labels_name_presets_and_customs() {
    assert_eq!(GameViewSize::Free.label(), "Free Aspect");
    assert_eq!(GameViewSize::Aspect(16, 9).label(), "16:9");
    assert_eq!(GameViewSize::Fixed(1280, 720).label(), "1280x720");
    assert_eq!(GameViewSize::Fixed(2193, 1080).label(), "2193x1080");
    assert_eq!(GameViewSize::Aspect(21, 9).label(), "21:9");
}

/// A script's set reads back at once and reaches the editor at its frame; the
/// newest of a frame's sets wins, and the editor's own pick is mirrored back.
#[test]
fn mailbox_round_trips() {
    let mut app = Application::new();
    assert!(!app.game_view_host());
    app.mark_game_view_host();
    assert!(app.game_view_host());
    let fixed = setting(GameViewSize::Fixed(1280, 720), false);
    app.request_game_view(setting(GameViewSize::Aspect(4, 3), true));
    app.request_game_view(fixed);
    assert_eq!(app.game_view_setting(), fixed, "read back at once");
    let mut editor = GameViewSetting::default();
    app.sync_game_view(&mut editor);
    assert_eq!(editor, fixed, "the newest set wins");
    editor.size = GameViewSize::Free;
    app.sync_game_view(&mut editor);
    assert_eq!(app.game_view_setting().size, GameViewSize::Free, "mirrored");
}

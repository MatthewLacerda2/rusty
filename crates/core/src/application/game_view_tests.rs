use super::*;

fn setting(size: GameViewSize, low_resolution: bool) -> GameViewSetting {
    GameViewSetting {
        size,
        low_resolution,
    }
}

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

#[test]
fn fixed_is_exact() {
    for low in [false, true] {
        let fixed = setting(GameViewSize::Fixed(1920, 1080), low);
        assert_eq!(fixed.resolve((640, 2000), 2.0), Some((1920, 1080)));
    }
}

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

#[test]
fn validity_bounds_both_sides() {
    assert!(GameViewSize::Free.is_valid());
    assert!(GameViewSize::Fixed(1, MAX_GAME_VIEW_SIDE).is_valid());
    assert!(!GameViewSize::Fixed(0, 10).is_valid());
    assert!(!GameViewSize::Aspect(16, MAX_GAME_VIEW_SIDE + 1).is_valid());
}

#[test]
fn labels_name_presets_and_customs() {
    assert_eq!(GameViewSize::Free.label(), "Free Aspect");
    assert_eq!(GameViewSize::Aspect(16, 9).label(), "16:9");
    assert_eq!(GameViewSize::Fixed(1280, 720).label(), "1280x720");
    assert_eq!(GameViewSize::Fixed(2193, 1080).label(), "2193x1080");
    assert_eq!(GameViewSize::Aspect(21, 9).label(), "21:9");
}

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

#[test]
fn parse_reads_the_three_spellings() {
    assert_eq!(GameViewSize::parse("Free"), Some(GameViewSize::Free));
    assert_eq!(
        GameViewSize::parse("16:9"),
        Some(GameViewSize::Aspect(16, 9))
    );
    assert_eq!(
        GameViewSize::parse(" 1280x720 "),
        Some(GameViewSize::Fixed(1280, 720))
    );
    for bad in [
        "", "free", "16", "0x720", "16:0", "1x2:3", "-1x5", "9000x10", "axb",
    ] {
        assert_eq!(GameViewSize::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn display_scale_follows_the_density() {
    assert_eq!(setting(GameViewSize::Free, true).display_scale(2.0), 1.0);
    assert_eq!(
        setting(GameViewSize::Aspect(4, 3), true).display_scale(2.0),
        1.0
    );
    // A fixed size is a pixel count: 1 on a Retina window, toggle or not.
    for low in [true, false] {
        let fixed = setting(GameViewSize::Fixed(800, 600), low);
        assert_eq!(fixed.display_scale(2.0), 1.0);
    }
    assert_eq!(setting(GameViewSize::Free, false).display_scale(2.0), 2.0);
    assert_eq!(setting(GameViewSize::Free, true).display_scale(0.5), 0.5);
}

#[test]
fn letterbox_centres_the_aspect() {
    // A 16:9 image in a square panel: full width, bars above and below.
    let ((x, y), (w, h)) = super::letterbox((160.0, 160.0), (16, 9));
    assert_eq!((x, w, h), (0.0, 160.0, 90.0));
    assert_eq!(y, 35.0);
    // A 4:3 image in a wide panel: full height, bars left and right.
    let ((x, y), (w, h)) = super::letterbox((400.0, 120.0), (4, 3));
    assert_eq!((y, w, h), (0.0, 160.0, 120.0));
    assert_eq!(x, 120.0);
}

#[test]
fn a_fixed_size_matches_the_display_at_scale_1() {
    use crate::settings::render_scale::RenderScaleDefault;
    let fixed = setting(GameViewSize::Fixed(1920, 1080), false);
    let scale = RenderScaleDefault::MatchDisplay.resolve(fixed.display_scale(2.0));
    assert_eq!(scale.get(), 1.0);
}

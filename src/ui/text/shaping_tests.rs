//! Shaped layout (#1328): determinism, ligatures and marks through `layout_text`.

use glam::Vec2;

use super::layout_text;
use crate::components::TextComponent;

fn text(s: &str) -> TextComponent {
    TextComponent {
        text: s.into(),
        font_size: 24.0,
        ..Default::default()
    }
}

#[test]
fn the_same_font_bytes_and_string_give_an_identical_layout() {
    let s = "Wavy fiords — AVATAR naïve e\u{301} <b>bold</b>\nsecond line";
    let rect = Vec2::new(180.0, 400.0);
    let a = layout_text(&text(s), rect);
    assert_eq!(a, layout_text(&text(s), rect), "twice in one process");
    // The same bytes under another path: a separate parse, the same layout.
    let bytes = include_bytes!("../../../engine/fonts/InstrumentSans-Regular.ttf");
    let path = crate::test_temp::dir().join("rusty_1328.ttf");
    std::fs::write(&path, bytes).expect("temp font");
    let copy = TextComponent {
        font: Some(path.to_string_lossy().into_owned()),
        ..text(s)
    };
    let b = layout_text(&copy, rect);
    let _ = std::fs::remove_file(&path);
    assert_eq!(a.glyphs, b.glyphs);
    assert_eq!((a.size, a.lines), (b.size, b.lines));
}

#[test]
fn a_ligature_draws_fewer_glyphs_than_it_has_letters_or_the_same() {
    let fi = layout_text(&text("fi"), Vec2::new(500.0, 100.0));
    let f_i = layout_text(&text("f i"), Vec2::new(500.0, 100.0));
    assert!(fi.glyphs.len() <= 2 && f_i.glyphs.len() == 2);
    assert!(fi.size.x > 0.0);
}

#[test]
fn a_combining_mark_never_wraps_away_from_its_base() {
    // A decomposed accent the font may compose or attach: either way its glyphs
    // stay on one line however narrow the rect.
    let l = layout_text(&text("e\u{301}"), Vec2::new(1.0, 500.0));
    assert_eq!(l.lines, 1, "{l:?}");
}

#[test]
fn an_empty_or_newline_only_text_lays_out_without_glyphs() {
    let rect = Vec2::new(100.0, 100.0);
    assert!(layout_text(&text(""), rect).glyphs.is_empty());
    let l = layout_text(&text("\n\n"), rect);
    assert!(l.glyphs.is_empty());
    assert_eq!(l.lines, 3);
}

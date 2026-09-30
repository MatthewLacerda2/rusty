//! CPU text layout: wrapping, overflow, ellipsis, auto-size, alignment (#419).

use glam::Vec2;

use super::*;
use crate::components::{TextAlignment, TextOverflow};
use crate::ui::text::font;

fn text(s: &str, size: f32) -> TextComponent {
    TextComponent {
        text: s.into(),
        font_size: size,
        ..Default::default()
    }
}

/// Glyph count per line, top to bottom, by distinct baselines.
fn line_glyphs(l: &TextLayout) -> Vec<usize> {
    let mut rows: Vec<(f32, usize)> = Vec::new();
    for g in &l.glyphs {
        match rows
            .iter_mut()
            .find(|(y, _)| (*y - g.origin.y).abs() < 1e-3)
        {
            Some(r) => r.1 += 1,
            None => rows.push((g.origin.y, 1)),
        }
    }
    rows.into_iter().map(|(_, n)| n).collect()
}

#[test]
fn wrapping_breaks_at_spaces_and_keeps_every_letter() {
    let t = text("alpha beta gamma", 20.0);
    let one = layout_text(&t, Vec2::new(1000.0, 200.0));
    assert_eq!(one.lines, 1);
    let narrow = one.size.x * 0.6;
    let l = layout_text(&t, Vec2::new(narrow, 200.0));
    assert_eq!(l.lines, 2, "wraps once");
    assert_eq!(line_glyphs(&l), vec![9, 5], "'alpha beta' / 'gamma'");
    assert!(l.size.x <= narrow + 1e-3);
    assert_eq!(l.glyphs.len(), 14, "spaces draw nothing, letters all do");
}

#[test]
fn a_word_wider_than_the_rect_breaks_between_characters() {
    let t = text("abcdefghij", 20.0);
    let full = layout_text(&t, Vec2::new(1000.0, 500.0)).size.x;
    let l = layout_text(&t, Vec2::new(full * 0.45, 500.0));
    assert!(l.lines >= 3);
    assert_eq!(l.glyphs.len(), 10);
}

#[test]
fn unwrapped_text_only_breaks_at_newlines() {
    let t = TextComponent {
        wrap: false,
        ..text("one two\nthree", 20.0)
    };
    let l = layout_text(&t, Vec2::new(10.0, 500.0));
    assert_eq!(l.lines, 2);
}

#[test]
fn truncate_drops_lines_that_do_not_fit() {
    let t = TextComponent {
        overflow: TextOverflow::Truncate,
        ..text("a\nb\nc\nd", 20.0)
    };
    let tall = layout_text(&t, Vec2::new(200.0, 1000.0));
    let per_line = tall.size.y / 4.0;
    let l = layout_text(&t, Vec2::new(200.0, per_line * 2.5));
    assert_eq!(l.lines, 2);
    assert!(l.truncated);
    assert!(l.size.y <= per_line * 2.5);
}

#[test]
fn ellipsis_ends_the_last_kept_line() {
    let t = TextComponent {
        overflow: TextOverflow::Ellipsis,
        wrap: false,
        ..text("A long status line that will not fit", 20.0)
    };
    let full = layout_text(&t, Vec2::new(2000.0, 100.0));
    let w = full.size.x * 0.5;
    let l = layout_text(&t, Vec2::new(w, 100.0));
    assert!(l.truncated);
    assert!(l.size.x <= w + 1e-3, "{} fits {w}", l.size.x);
    let font = font::default_font();
    let dots = font.glyph('…').expect("the default font has an ellipsis");
    assert_eq!(l.glyphs.last().map(|g| g.glyph), Some(dots));
}

#[test]
fn overflow_draws_everything() {
    let t = text("a\nb\nc", 20.0);
    let l = layout_text(&t, Vec2::new(5.0, 5.0));
    assert_eq!((l.lines, l.truncated), (3, false));
}

#[test]
fn auto_size_picks_the_largest_size_that_fits() {
    let t = TextComponent {
        auto_size: true,
        auto_size_min: 8.0,
        auto_size_max: 200.0,
        ..text("Ammo 30 / 120", 36.0)
    };
    let rect = Vec2::new(300.0, 60.0);
    let l = layout_text(&t, rect);
    assert!(l.font_size > 8.0 && l.font_size < 200.0);
    assert!(l.size.x <= rect.x + 1e-3 && l.size.y <= rect.y + 1e-3);
    let bigger = TextComponent {
        auto_size: false,
        wrap: true,
        font_size: l.font_size * 1.05,
        ..t.clone()
    };
    let big = layout_text(&bigger, rect);
    assert!(
        big.size.y > rect.y || big.size.x > rect.x,
        "5% larger no longer fits"
    );
    let roomy = layout_text(&t, Vec2::new(5000.0, 5000.0));
    assert_eq!(roomy.font_size, 200.0, "caps at the maximum");
}

#[test]
fn alignment_places_the_block_in_the_rect() {
    let rect = Vec2::new(400.0, 200.0);
    let at = |alignment| {
        let t = TextComponent {
            alignment,
            ..text("Hi", 30.0)
        };
        let l = layout_text(&t, rect);
        (l.glyphs[0].origin, l.size)
    };
    let (tl, size) = at(TextAlignment::TopLeft);
    let (mc, _) = at(TextAlignment::MiddleCenter);
    let (br, _) = at(TextAlignment::BottomRight);
    assert!(tl.x.abs() < 3.0 && tl.y > rect.y * 0.7);
    assert!((mc.x - (rect.x - size.x) * 0.5).abs() < 1e-3);
    assert!(br.x > rect.x - size.x - 3.0 && br.y < 30.0);
}

#[test]
fn rich_tags_size_color_and_style_glyphs() {
    let t = text(
        "a<size=60>b</size><b>c</b><i>d</i><color=#ff0000>e</color>",
        20.0,
    );
    let l = layout_text(&t, Vec2::new(500.0, 200.0));
    let g = &l.glyphs;
    assert_eq!(g.len(), 5);
    assert_eq!((g[0].size, g[1].size), (20.0, 60.0));
    assert!(g[2].faux_bold && g[3].faux_italic);
    assert_eq!(g[4].color.x, 1.0);
}

#[test]
fn preferred_size_measures_unwrapped_width_and_wrapped_height() {
    let t = text("alpha beta gamma", 20.0);
    let wide = preferred_size(&t, 10_000.0);
    let narrow = preferred_size(&t, wide.x * 0.6);
    assert_eq!(wide.x, narrow.x, "width is always the unwrapped width");
    assert!(narrow.y > wide.y * 1.8, "height follows the wrap");
}

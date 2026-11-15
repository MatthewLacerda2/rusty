//! Lettering's layout: centred blocks, alignment, tracking.

use super::*;

fn layout(align: Align) -> Layout {
    Layout {
        size: 0.1,
        tracking: 0.0,
        line_spacing: 1.2,
        align,
    }
}

/// The box of every glyph point.
fn bounds(shapes: &[Shape]) -> (Vec2, Vec2) {
    let pts = shapes.iter().flatten().flatten();
    pts.fold((Vec2::splat(9.0), Vec2::splat(-9.0)), |(lo, hi), p| {
        (lo.min(*p), hi.max(*p))
    })
}

#[test]
fn a_block_is_centred_and_lines_align_inside_it() {
    let f = rusty_font::default_font();
    let one = glyphs(&f, "HH", layout(Align::Center));
    let (lo, hi) = bounds(&one);
    assert!(((lo.x + hi.x) / 2.0 - 0.5).abs() < 0.01, "{lo} {hi}");
    let two = |a| glyphs(&f, "HHHH\nH", layout(a));
    let last = |s: Vec<Shape>| bounds(&s[4..]).0.x;
    let (l, c, r) = (
        last(two(Align::Left)),
        last(two(Align::Center)),
        last(two(Align::Right)),
    );
    assert!(l < c && c < r, "{l} {c} {r}");
    let first = bounds(&two(Align::Left)[..4]).0.x;
    assert!((l - first).abs() < 1e-4, "left lines share an edge");
    let (lo, hi) = bounds(&two(Align::Center));
    assert!(
        ((lo.y + hi.y) / 2.0 - 0.5).abs() < 0.03,
        "the block is centred down: {lo} {hi}"
    );
    assert!(hi.y - lo.y > 0.15, "two lines are taller than one");
}

#[test]
fn tracking_spreads_glyphs_and_empty_text_draws_nothing() {
    let f = rusty_font::default_font();
    let tight = bounds(&glyphs(&f, "HHH", layout(Align::Center)));
    let loose = Layout {
        tracking: 0.5,
        ..layout(Align::Center)
    };
    let wide = bounds(&glyphs(&f, "HHH", loose));
    assert!(((wide.1.x - wide.0.x) - (tight.1.x - tight.0.x) - 2.0 * 0.05).abs() < 1e-3);
    assert!(glyphs(&f, "", layout(Align::Center)).is_empty());
    assert!(glyphs(&f, " \n ", layout(Align::Center)).is_empty());
}

#[test]
fn ligatures_are_shaped() {
    let f = rusty_font::default_font();
    assert_eq!(glyphs(&f, "fi", layout(Align::Center)).len(), 1);
}

#[test]
fn a_crlf_line_break_draws_as_a_newline() {
    let f = rusty_font::default_font();
    let lf = glyphs(&f, "EXIT\nNO 3", layout(Align::Center));
    let crlf = glyphs(&f, "EXIT\r\nNO 3", layout(Align::Center));
    assert_eq!(lf, crlf, "no `\\r` glyph, and the same centring");
}

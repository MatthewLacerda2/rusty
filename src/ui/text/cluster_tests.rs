//! Clusters through layout (#1328): a mark and a ligature under letter spacing,
//! style tags, overflow cuts and synthesized styles.

use glam::Vec2;

use super::font;
use super::layout_text;
use crate::components::{TextComponent, TextOverflow};

const WIDE: Vec2 = Vec2::new(500.0, 100.0);

fn text(s: &str) -> TextComponent {
    TextComponent {
        text: s.into(),
        font_size: 24.0,
        ..Default::default()
    }
}

/// Each glyph's x, in order.
fn xs(t: &TextComponent) -> Vec<f32> {
    layout_text(t, WIDE)
        .glyphs
        .iter()
        .map(|g| g.origin.x)
        .collect()
}

#[test]
fn letter_spacing_and_faux_bold_widen_a_cluster_once_and_keep_its_mark_on_it() {
    let plain = xs(&text("q\u{301}x"));
    let spaced = xs(&TextComponent {
        letter_spacing: 0.25,
        ..text("q\u{301}x")
    });
    let bold = xs(&text("<b>q\u{301}</b>x"));
    for run in [&spaced, &bold] {
        assert_eq!(run.len(), 3);
        assert!(
            (run[1] - run[0] - (plain[1] - plain[0])).abs() < 1e-4,
            "{run:?}"
        );
    }
    assert!(
        (spaced[2] - plain[2] - 0.25 * 24.0).abs() < 1e-3,
        "one gap, not two"
    );
}

#[test]
fn a_style_tag_inside_a_ligature_keeps_the_letters_apart_and_kerning_across_it() {
    let red = layout_text(&text("f<color=#ff0000>i</color>"), WIDE);
    assert_eq!(red.glyphs.len(), 2, "the tag still shows");
    assert_eq!(red.glyphs[1].color, glam::Vec4::new(1.0, 0.0, 0.0, 1.0));
    assert_ne!(red.glyphs[0].color, red.glyphs[1].color);
    let office = layout_text(&text("of<b>fi</b>ce"), WIDE);
    let bolds: Vec<bool> = office.glyphs.iter().map(|g| g.faux_bold).collect();
    assert_eq!(bolds, [false, false, true, true, false, false]);
    let kerned = layout_text(&text("AV"), WIDE).size.x;
    let tagged = layout_text(&text("A<color=#ff0000>V</color>"), WIDE).size.x;
    assert!((kerned - tagged).abs() < 1e-4, "{kerned} vs {tagged}");
}

#[test]
fn an_ellipsis_never_drops_a_mark_alone() {
    let f = font::default_font();
    let e = f.glyph('e').expect("e");
    let line = "cafe\u{301} cafe\u{301} cafe\u{301}";
    for w in (10..200).step_by(3) {
        let t = TextComponent {
            wrap: false,
            overflow: TextOverflow::Ellipsis,
            ..text(line)
        };
        let g = layout_text(&t, Vec2::new(w as f32, 100.0)).glyphs;
        for (i, p) in g.iter().enumerate().filter(|(_, p)| p.glyph == e) {
            let next = g.get(i + 1).map(|n| n.origin.y);
            assert!(
                next.is_some_and(|y| y > p.origin.y),
                "w {w}: e lost its accent"
            );
        }
    }
}

#[test]
fn a_cut_line_drops_the_kerning_against_the_cut_glyph() {
    let lone_a = layout_text(&text("A"), WIDE).size.x;
    let t = TextComponent {
        wrap: false,
        overflow: TextOverflow::Ellipsis,
        ..text("AVATAR")
    };
    let cut = (1..400)
        .map(|w| layout_text(&t, Vec2::new(w as f32 * 0.25, 100.0)).glyphs)
        .find(|g| g.len() == 2)
        .expect("a width that keeps A…");
    assert!(
        (cut[1].origin.x - cut[0].origin.x - lone_a).abs() < 1e-3,
        "{cut:?}"
    );
}

#[test]
fn an_accent_on_a_space_is_drawn_and_a_mark_carries_its_lines_baseline() {
    assert_eq!(layout_text(&text(" \u{301}"), WIDE).glyphs.len(), 1);
    assert_eq!(layout_text(&text("a \u{308}"), WIDE).glyphs.len(), 2);
    // Faux italic shears about `baseline` (the renderer's test covers the shear).
    let g = layout_text(&text("<i>q\u{301}</i>\nq\u{301}"), WIDE).glyphs;
    assert!(g[1].faux_italic && g[0].baseline == g[1].baseline);
    assert!(g[2].baseline < g[0].baseline && g[2].baseline == g[3].baseline);
}

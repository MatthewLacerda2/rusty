//! Every way a file names a sibling crate or the lib, folded: each form the fold
//! rewrites, and each it must leave and report.

use super::super::fold;

fn members() -> Vec<(String, String)> {
    vec![
        ("rusty_font".to_string(), "font".to_string()),
        ("rusty_ecs".to_string(), "ecs".to_string()),
    ]
}

fn folded(src: &str) -> String {
    fold::caller(src, "ui", "rusty", false, &members())
}

fn left(src: &str) -> Vec<(usize, String)> {
    fold::unfoldable(src, "ui", "rusty", &members())
        .into_iter()
        .map(|(line, l)| (line, l.name))
        .collect()
}

#[test]
fn every_bare_form_of_a_member_folds_to_its_module() {
    let cases = [
        ("use rusty_font as font;", "use crate::font as font;"),
        ("use rusty_font;", "use crate::font;"),
        ("use ::rusty_font as f;", "use crate::font as f;"),
        ("extern crate rusty_font;", "use crate::font;"),
        ("extern crate rusty_font as f;", "use crate::font as f;"),
        (
            "use {rusty_font as font, rusty_ecs};",
            "use {crate::font as font, crate::ecs};",
        ),
        ("#[serde(with = rusty_ecs)]", "#[serde(with = crate::ecs)]"),
        (
            "m!(rusty_font, rusty_ecs::W);",
            "m!(crate::font, crate::ecs::W);",
        ),
        ("use rusty as r;", "use crate as r;"),
    ];
    for (src, want) in cases {
        assert_eq!(folded(&format!("{src}\n")), format!("{want}\n"), "{src}");
        assert!(left(src).is_empty(), "{src} folds, so nothing is left");
    }
}

#[test]
fn a_name_that_is_no_crate_stays_as_written() {
    let src = "let p = \"rusty_font\"; // rusty_font\n\
               s.rusty_font; $rusty_font; 'rusty_font: loop {}\n\
               rusty_font!(); rusty_fonts(); trusty_font; r#rusty_font;\n\
               /* rusty_font\n rusty_font */ let r = 0..rusty_ecs::N;\n";
    let want = "let p = \"rusty_font\"; // rusty_font\n\
                s.rusty_font; $rusty_font; 'rusty_font: loop {}\n\
                rusty_font!(); rusty_fonts(); trusty_font; r#rusty_font;\n\
                /* rusty_font\n rusty_font */ let r = 0..crate::ecs::N;\n";
    assert_eq!(folded(src), want);
    assert!(left(src).is_empty());
}

#[test]
fn a_form_the_fold_cannot_rewrite_is_left_and_reported() {
    let src = "use rusty_ecs::W;\npub use rusty_font as font;\nuse rusty;\nuse {rusty, x};\n";
    assert_eq!(
        left(src),
        [
            (2, "rusty_font".to_string()),
            (3, "rusty".to_string()),
            (4, "rusty".to_string()),
        ]
    );
    assert_eq!(
        folded(src),
        "use crate::ecs::W;\npub use rusty_font as font;\nuse rusty;\nuse {rusty, x};\n"
    );
    assert!(left("pub use rusty_font::{Glyph, Font};\n").is_empty());
}

/// A bare `rusty` or `rusty_<m>` outside a crate position is a variable, a
/// field, a parameter or a later path segment: folded as written, never
/// reported.
#[test]
fn a_bare_name_outside_a_crate_position_is_left_and_not_reported() {
    let src = "let rusty = 1;\nfn f(rusty: u32, rusty_font: u8) {}\n\
               let s = Foo { rusty: 1, rusty_font };\nf(rusty);\nlet v = rusty;\n\
               let w = vec![rusty, x];\nlet k = Foo { rusty, .. };\n\
               let g = Self::rusty_font;\nlet h = Self::rusty_font::X;\n\
               g(rusty_font, rusty);\n";
    assert_eq!(folded(src), src);
    assert!(left(src).is_empty());
}

#[test]
fn a_crate_position_spans_lines_and_attributes() {
    let cases = [
        (
            "use {\n    rusty_font,\n    rusty_ecs::W,\n};\n",
            "use {\n    crate::font,\n    crate::ecs::W,\n};\n",
        ),
        (
            "use rusty_font::{rusty_ecs, x};\nlet y = rusty_ecs;\n",
            "use crate::font::{rusty_ecs, x};\nlet y = rusty_ecs;\n",
        ),
        (
            "#[cfg(test)] use rusty_font;\n",
            "#[cfg(test)] use crate::font;\n",
        ),
        (
            "use rusty_font::{self, X};\n",
            "use crate::font::{self, X};\n",
        ),
        (
            "pub(crate) use rusty_font as f;\n",
            "pub(crate) use crate::font as f;\n",
        ),
    ];
    for (src, want) in cases {
        assert_eq!(folded(src), want, "{src}");
        assert!(left(src).is_empty(), "{src}");
    }
}

/// Every spelling of a whole-crate re-export, and the lib's `{self}`: the item,
/// not the line, decides `pub`.
#[test]
fn a_whole_crate_reexport_or_the_lib_alone_is_reported_in_any_spelling() {
    let cases = [
        "pub use {\n    rusty_ecs::W,\n    rusty_font,\n};\n",
        "pub extern crate rusty_font;\n",
        "#[doc(hidden)] pub use rusty_font as f;\n",
        "pub use rusty_font::{self as x};\n",
        "pub use rusty_font::{\n    W,\n    self,\n};\n",
        "use rusty::{self};\n",
        "use rusty::{self, X};\n",
    ];
    for src in cases {
        assert_eq!(left(src).len(), 1, "{src}");
    }
    assert_eq!(left("pub use rusty_font::{a::{self}, b};\n"), []);
}

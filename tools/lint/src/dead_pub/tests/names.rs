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

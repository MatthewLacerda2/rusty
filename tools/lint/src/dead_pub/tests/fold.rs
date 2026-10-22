use super::super::fold::{self, Bin};
use super::CARGO;

#[test]
fn reads_bins_and_their_features() {
    let bins = fold::bins(CARGO);
    let play = Bin {
        name: "play-x".into(),
        path: "src/bin/play.rs".into(),
        features: vec!["dev".into()],
    };
    assert_eq!(bins, [play]);
    assert_eq!(bins[0].module(), "__bin_play_x");
}

#[test]
fn lib_goes_crate_private_and_folds_its_callers() {
    let lib = fold::lib(
        "pub mod a;\n#[cfg(test)]\nmod t;\n",
        &fold::bins(CARGO),
        Some("tests/main.rs"),
    );
    assert!(lib.starts_with("pub(crate) mod a;\n#[cfg(test)]\nmod t;\n"));
    assert!(lib.contains(
        "#[cfg(all(feature = \"dev\"))]\n#[path = \"bin/play.rs\"]\npub mod __bin_play_x;\n"
    ));
    assert!(lib.ends_with("#[cfg(test)]\n#[path = \"../tests/main.rs\"]\nmod __itests;\n"));
}

#[test]
fn a_caller_names_the_lib_as_crate_and_keeps_its_line_numbers() {
    let src = "#![cfg_attr(\n  x, y\n)]\nuse fixture::a;\nmod m;\nuse crate::m::f;\nfn main() {}\n";
    let out = fold::caller(src, "__bin_p", "fixture", true);
    assert_eq!(
        out,
        "\n\n\nuse crate::a;\nmod m;\nuse crate::__bin_p::m::f;\npub fn main() {}\n"
    );
}

#[test]
fn an_inner_attribute_inside_a_macro_body_survives_the_fold() {
    let src = "//! doc\n#![allow(x)]\n/* c */\nproptest! {\n    #![proptest_config(config())]\n}\n";
    let out = fold::caller(src, "__itests", "fixture", true);
    assert_eq!(
        out,
        "//! doc\n\n/* c */\nproptest! {\n    #![proptest_config(config())]\n}\n"
    );
}

#[test]
fn a_root_counts_brackets_outside_string_literals() {
    let src = "#![doc = \"see [x\"]\nuse crate::a;\n";
    assert_eq!(
        fold::caller(src, "m", "fixture", true),
        "\nuse crate::m::a;\n"
    );
}

#[test]
fn a_submodule_keeps_its_inner_attributes() {
    let src = "#![cfg(target_os = \"macos\")]\nuse crate::a;\n";
    assert_eq!(
        fold::caller(src, "m", "fixture", false),
        "#![cfg(target_os = \"macos\")]\nuse crate::m::a;\n"
    );
}

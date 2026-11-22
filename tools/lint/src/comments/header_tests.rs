use super::{check, words, ECHO, PATH};
use crate::comments::{split, Kind};

fn rules(path: &str, source: &str) -> Vec<&'static str> {
    let kind = Kind::of(path).expect("a comment language");
    let lines = split(kind, source);
    check(path, kind, source, &lines)
        .iter()
        .map(|f| f.rule)
        .collect()
}

#[test]
fn a_header_opening_with_its_own_path_is_flagged() {
    for (path, source) in [
        (
            "crates/asset/src/manifest_tests.rs",
            "//! crates/asset/src/manifest_tests.rs — the manifest round-trips.\n",
        ),
        (
            "engine/shaders/x.wgsl",
            "\n// `engine/shaders/x.wgsl`: a blur.\nfn f() {}\n",
        ),
        (
            "games/a/scripts/b.lua",
            "-- scripts/b.lua: the door opens on use.\n",
        ),
        (
            "crates/a/src/b.rs",
            "//! b.rs: the order the passes run in.\n",
        ),
    ] {
        assert_eq!(rules(path, source), [PATH], "{source}");
    }
}

#[test]
fn a_path_named_mid_sentence_or_another_files_path_passes() {
    for (path, source) in [
        (
            "crates/a/src/b.rs",
            "//! Reads `crates/a/src/b.rs`'s twin, in order.\n",
        ),
        (
            "crates/a/src/b.rs",
            "//! `c.rs` holds the rest, sorted by name.\n",
        ),
        (
            "crates/a/src/b.rs",
            "// lint: allow(comment-path) quoted\n//! b.rs: as named.\n",
        ),
    ] {
        assert!(rules(path, source).is_empty(), "{source}");
    }
}

#[test]
fn a_one_line_module_doc_that_only_names_its_module_is_flagged() {
    let names = "pub struct Bar;\nfn command() {}\n";
    let path = "crates/dev/src/bake/progress.rs";
    let echo = format!("//! The bake command's progress bar.\n{names}");
    assert_eq!(rules(path, &echo), [ECHO]);
    assert_eq!(
        rules(path, "//! See `docs/api/Bake.md`.\nfn f() {}\n"),
        [ECHO]
    );
    let plural = "//! Progress bars.\nstruct Bar;\n";
    assert_eq!(rules(path, plural), [ECHO], "a plural is its name");
    for (path, doc) in [
        (
            "crates/render/src/draw/camera/stages.rs",
            "//! The camera's draw stages.\nfn stage() {}\n",
        ),
        ("crates/ui/src/boxes.rs", "//! Boxes.\nstruct Box;\n"),
        ("crates/physics/src/body.rs", "//! The bodies.\nfn f() {}\n"),
    ] {
        assert_eq!(
            rules(path, doc),
            [ECHO],
            "{doc}: a plural folded on `es` and `ies`"
        );
    }
    let below = "// SPDX-free.\n//! Progress.\nfn f() {}\n";
    assert_eq!(rules(path, below), [ECHO], "below a plain comment");
}

#[test]
fn a_module_doc_stating_more_than_its_names_passes() {
    let path = "crates/dev/src/bake/progress.rs";
    for source in [
        "//! The bar's share of the bake, in percent, never past 100.\nstruct Bar;\n",
        "//! Progress, drawn after the steps run.\nfn progress() {}\n",
        "//! The bake's progress.\n//! Read once a frame.\nfn f() {}\n",
        "//! Progress bars, see `docs/ui.md` for their colours.\nstruct Bar;\n",
        "// lint: allow(comment-echo) the crate root's one line\n//! Progress.\nfn f() {}\n",
        "fn f() {}\n//! Progress.\n",
        "//! A bake stops part way.\nfn a_bake_stops_part_way() {}\n",
        "//! The bake's stags.\nstruct Stage;\n",
    ] {
        assert!(rules(path, source).is_empty(), "{source}");
    }
    assert!(rules("x.lua", "--! Progress.\n").is_empty(), "Rust only");
}

#[test]
fn words_split_names_and_prose() {
    assert_eq!(
        words("ProgressBar bake_step's `docs/x.md` HTTP2"),
        ["progress", "bar", "bake", "step", "docs", "x", "md", "http2"]
    );
}

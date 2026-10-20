use super::fold::{self, Bin};
use super::report;
use std::fs;
use std::path::PathBuf;

const CARGO: &str = "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
[workspace]\n\n[features]\ndev = []\n\n[[bin]]\nname = \"play-x\"\npath = \"src/bin/play.rs\"\n\
required-features = [\"dev\"]\n";

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
    let out = fold::caller(src, "__bin_p", "fixture");
    assert_eq!(
        out,
        "\n\n\nuse crate::a;\nmod m;\nuse crate::__bin_p::m::f;\npub fn main() {}\n"
    );
}

#[test]
fn parses_only_unused_items_and_notices_errors() {
    let stderr = "src/a.rs:3:8: warning: function `f` is never used\n\
                  src/b.rs:1:5: warning: unused import: `x`\n\
                  src/c.rs:2:1: warning: missing docs\n";
    let (found, failed) = report::parse(stderr);
    assert_eq!(found.len(), 2);
    assert!(!failed);
    assert!(report::parse("src/a.rs:1:1: error[E0425]: nope\n").1);
}

/// A crate on disk: the must-flag and must-pass cases, through real `cargo check`.
fn fixture() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lint_dead_pub_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let file = |path: &str, body: &str| {
        let p = dir.join("crate").join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    };
    file("Cargo.toml", CARGO);
    file("src/lib.rs", "pub mod a;\n");
    file(
        "src/a.rs",
        "pub fn nobody() {}\npub fn the_bin() {}\npub fn the_tests() {}\n",
    );
    file(
        "src/bin/play.rs",
        "#![allow(unused)]\nfn main() {\n    fixture::a::the_bin();\n}\n",
    );
    file(
        "tests/main.rs",
        "#[test]\nfn t() {\n    fixture::a::the_tests();\n}\n",
    );
    dir
}

#[test]
fn flags_a_pub_fn_nothing_calls_and_passes_one_only_a_bin_calls() {
    let dir = fixture();
    let tree = dir.join("tree");
    super::prepare(&dir.join("crate"), &tree, "fixture").unwrap();
    let target = dir.join("target");
    let with = super::check(&tree, &target, &["--profile", "test"], &["dev"]).unwrap();
    let without = super::check(&tree, &target, &[], &["dev"]).unwrap();
    let body = report::render(&with, &without, &|p| super::platform::is_gated(&tree, p));
    let _ = fs::remove_dir_all(&dir);
    let dead = body.split("Only tests").next().unwrap();
    assert!(dead.contains("`nobody`"), "{body}");
    assert!(!body.contains("the_bin"), "{body}");
    assert!(
        body.split("Only tests")
            .nth(1)
            .unwrap()
            .contains("`the_tests`"),
        "{body}"
    );
}

#[test]
fn platform_cfg_is_found_in_a_file_and_in_its_declaring_mod() {
    use super::platform::{is_gated, mentions_platform};
    assert!(mentions_platform(
        "#[cfg(target_os = \"macos\")]\nfn f() {}"
    ));
    assert!(mentions_platform("#[cfg(not(windows))]\nfn f() {}"));
    assert!(!mentions_platform("// cfg(unix)\nfn f() {}\n#[cfg(test)]"));
    let dir = std::env::temp_dir().join(format!("lint_dead_pub_gated_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src/s")).unwrap();
    let put = |p: &str, b: &str| fs::write(dir.join(p), b).unwrap();
    put(
        "src/s/mod.rs",
        "#[cfg(target_os = \"macos\")]\nmod mac;\nmod plain;\n",
    );
    put("src/s/mac.rs", "pub fn dock() {}\n");
    put("src/s/plain.rs", "pub fn nothing() {}\n");
    put("src/s/own.rs", "#[cfg(unix)]\npub fn u() {}\n");
    let flagged = is_gated(&dir, "src/s/mac.rs") && is_gated(&dir, "src/s/own.rs");
    let passed = !is_gated(&dir, "src/s/plain.rs");
    let _ = fs::remove_dir_all(&dir);
    assert!(flagged, "a platform-gated file must be flagged");
    assert!(passed, "a plain file must pass");
}

#[test]
fn a_gated_finding_gets_its_own_heading() {
    let w = |path: &str| report::Warning {
        path: path.into(),
        line: 1,
        message: "function `f` is never used".into(),
    };
    let set: std::collections::BTreeSet<_> = [w("src/mac.rs"), w("src/plain.rs")].into();
    let body = report::render(&set, &set, &|p| p == "src/mac.rs");
    let (before, gated) = body.split_once("Platform-gated").unwrap();
    assert!(
        before.contains("src/plain.rs") && !before.contains("src/mac.rs"),
        "{body}"
    );
    assert!(
        gated.contains("src/mac.rs") && !gated.contains("src/plain.rs"),
        "{body}"
    );
}

//! The workspace fold through real `cargo check`: a member's `pub` fn nobody names
//! is flagged, one another member, a bin or a macro names passes.

use super::super::{check, prepare, report, workspace};
use std::fs;
use std::path::PathBuf;

const ROOT: &str = "[workspace]\nmembers = [\"crates/*\"]\ndefault-members = [\".\", \"crates/*\"]\n\n\
[workspace.dependencies]\nw-low = { path = \"crates/low\" }\nw-high = { path = \"crates/high\" }\n\
w-support = { path = \"crates/support\" }\n\n[package]\nname = \"w\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
[features]\ndev = [\"w-low/dev\"]\n\n[dependencies]\nw-low = { workspace = true }\nw-high = { workspace = true }\n\n\
[dev-dependencies]\nw-support = { workspace = true }\n\n[[bin]]\nname = \"b\"\npath = \"src/bin/b.rs\"\n";

fn member(name: &str, deps: &str) -> String {
    format!("[package]\nname = \"w-{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[features]\ndev = []\n\n[dependencies]\n{deps}\n")
}

fn fixture(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lint_dead_pub_ws_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let file = |path: &str, body: &str| {
        let p = dir.join("crate").join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    };
    file("Cargo.toml", ROOT);
    file(
        "src/lib.rs",
        "pub use w_low as low;\npub use w_high as high;\n",
    );
    file(
        "src/bin/b.rs",
        "fn main() {\n    w::low::by_bin();\n    w::high::run();\n}\n",
    );
    file("crates/low/Cargo.toml", &member("low", ""));
    file(
        "crates/low/src/lib.rs",
        "pub mod m;\npub fn nobody() {}\npub fn by_high() {}\npub fn by_bin() {}\npub fn by_macro() {}\n\
#[macro_export]\nmacro_rules! low_macro {\n    () => { $crate::by_macro() };\n}\n",
    );
    file(
        "crates/low/src/m.rs",
        "pub fn deep_nobody() {}\npub fn deep_used() {}\n",
    );
    file(
        "crates/high/Cargo.toml",
        &member("high", "w-low = { workspace = true }"),
    );
    file(
        "crates/high/src/lib.rs",
        "pub fn run() {\n    w_low::by_high();\n    ::w_low::m::deep_used();\n    w_low::low_macro!();\n}\n",
    );
    file("crates/support/Cargo.toml", &member("support", ""));
    file("crates/support/src/lib.rs", "pub fn helper() {}\n");
    file(
        "tests/main.rs",
        "#[test]\nfn t() {\n    w_support::helper();\n}\n",
    );
    dir
}

#[test]
fn reads_members_their_modules_and_macros() {
    let dir = fixture("read");
    let root = dir.join("crate");
    let lib = fs::read_to_string(root.join("src/lib.rs")).unwrap();
    let members = workspace::members(&root, &lib).unwrap();
    let _ = fs::remove_dir_all(&dir);
    let got: Vec<(&str, &str, bool)> = members
        .iter()
        .map(|m| (m.dir.as_str(), m.module.as_str(), m.test_only))
        .collect();
    assert_eq!(
        got,
        [
            ("crates/high", "high", false),
            ("crates/low", "low", false),
            ("crates/support", "w_support", true),
        ]
    );
    assert_eq!(members[1].macros, ["low_macro"]);
}

#[test]
fn flags_a_member_fn_nobody_names_and_passes_one_another_crate_names() {
    let dir = fixture("check");
    let tree = dir.join("tree");
    prepare(&dir.join("crate"), &tree, "w").unwrap();
    let target = dir.join("target");
    let with = check(&tree, &target, &["--profile", "test"], &["dev"]);
    let without = check(&tree, &target, &[], &["dev"]);
    let _ = fs::remove_dir_all(&dir);
    let body = report::render(&with.unwrap(), &without.unwrap(), &|_| false);
    let dead = body.split("Only tests").next().unwrap();
    for name in ["`nobody`", "`deep_nobody`"] {
        assert!(dead.contains(name), "{name} must be flagged:\n{body}");
    }
    assert!(
        dead.contains("crates/low/src/m.rs:1:"),
        "real paths:\n{body}"
    );
    for name in [
        "by_high",
        "by_bin",
        "by_macro",
        "deep_used",
        "`run`",
        "helper",
    ] {
        assert!(!body.contains(name), "{name} must pass:\n{body}");
    }
}

#[test]
fn a_fold_that_makes_nothing_private_is_an_error() {
    let dir = std::env::temp_dir().join(format!("lint_dead_pub_none_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let src = dir.join("crate");
    fs::create_dir_all(src.join("src")).unwrap();
    fs::write(src.join("Cargo.toml"), member("x", "")).unwrap();
    fs::write(src.join("src/lib.rs"), "pub use std as s;\n").unwrap();
    let blind = prepare(&src, &dir.join("tree"), "w_x");
    let _ = fs::remove_dir_all(&dir);
    let err = blind.expect_err("a fold with nothing to make private must fail");
    assert!(
        err.to_string().contains("made nothing crate-private"),
        "{err}"
    );
}

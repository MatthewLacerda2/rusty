//! The suite's own layout guard (#483).
//!
//! `tests/` compiles as one binary rooted at `tests/main.rs` (`autotests = false`), so
//! a file joins the suite only through a `mod` line in the file that owns its folder.
//! A forgotten line compiles nothing and fails nothing: that file's tests would just
//! never run. This walks `tests/` and names every file or folder nobody declares.

use std::fs;
use std::path::{Path, PathBuf};

/// Does `source` hold a `mod <name>;` line (attributes sit on their own lines)?
fn declares(source: &str, name: &str) -> bool {
    let wanted = format!("mod {name};");
    source.lines().any(|l| {
        let l = l.trim();
        l.strip_prefix("pub ").unwrap_or(l) == wanted
    })
}

/// Collect every `.rs` file and module folder under `dir` that `owner` (the file
/// declaring `dir`'s modules: `main.rs` at the root, `mod.rs` below) leaves out.
fn orphans(dir: &Path, owner: &str, out: &mut Vec<PathBuf>) {
    let source = fs::read_to_string(dir.join(owner)).unwrap_or_default();
    for entry in fs::read_dir(dir).expect("tests/ is readable").flatten() {
        let path = entry.path();
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let is_module_dir = path.join("mod.rs").is_file();
        let is_module_file = path.extension().is_some_and(|e| e == "rs")
            && path.file_name().is_some_and(|n| n != owner);
        if (is_module_dir || is_module_file) && !declares(&source, &stem) {
            out.push(path.clone());
        }
        if path.is_dir() {
            orphans(&path, "mod.rs", out);
        }
    }
}

#[test]
fn every_test_file_is_declared_as_a_module() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut out = Vec::new();
    orphans(&root, "main.rs", &mut out);
    assert!(
        out.is_empty(),
        "these test files are never compiled — add a `mod` line for each \
         (tests/main.rs, or the folder's mod.rs): {out:#?}"
    );
}

#[test]
fn declares_matches_plain_and_pub_mod_lines_only() {
    let src = "#[cfg(feature = \"dev\")]\nmod gpu;\n  pub mod physics_rapier;\n// mod gone;";
    assert!(declares(src, "gpu"));
    assert!(declares(src, "physics_rapier"));
    assert!(
        !declares(src, "gone"),
        "a commented-out line is not a declaration"
    );
    assert!(!declares(src, "phys"), "a prefix is not a declaration");
}

//! The size gate's caps and scans (`mod.rs`); Python's counting is tested beside it
//! in `python.rs`.

use super::*;
use std::path::Path;

/// A scratch tree under the temp dir, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        Scratch(std::env::temp_dir().join(format!("rusty-lint-{tag}-{}", std::process::id())))
    }

    fn file(&self, rel: &str, body: &str) -> PathBuf {
        let path = self.0.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, body).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

#[test]
fn normal_files_get_the_default_cap() {
    assert_eq!(limit_for(Path::new("src/render/mod.rs")), MAX_FILE_LINES);
}

#[test]
fn test_and_fixture_files_get_the_tight_cap() {
    assert_eq!(limit_for(Path::new("tests/foo.rs")), MAX_TEST_FILE_LINES);
    assert_eq!(limit_for(Path::new("src/bar_test.rs")), MAX_TEST_FILE_LINES);
    assert_eq!(
        limit_for(Path::new("src/fixtures/baz.rs")),
        MAX_TEST_FILE_LINES
    );
}

#[test]
fn one_binary_test_layout_keeps_the_tight_cap() {
    // `tests/` is one binary (#483): its root, a module folder's `mod.rs`, and the
    // files inside that folder are all still standalone test files.
    assert_eq!(limit_for(Path::new("tests/main.rs")), MAX_TEST_FILE_LINES);
    assert_eq!(
        limit_for(Path::new("tests/gpu/mod.rs")),
        MAX_TEST_FILE_LINES
    );
    assert_eq!(
        limit_for(Path::new("tests/gpu/fxaa_screenshot.rs")),
        MAX_TEST_FILE_LINES
    );
    // A `tests.rs` there is no source's sibling: it keeps the tight cap (#1367).
    assert_eq!(
        limit_for(Path::new("tests/x/tests.rs")),
        MAX_TEST_FILE_LINES
    );
    assert_eq!(limit_for(Path::new("src/x/tests.rs")), MAX_FILE_LINES);
}

#[test]
fn tests_sibling_gets_the_source_cap_not_the_tight_one() {
    // A `<x>_tests.rs` beside its `<x>.rs` is one source's single test home (issue
    // #211), so it shares the source cap: `bar.rs` + `bar_tests.rs` at 250 passes.
    let tmp = Scratch::new("960-sibling");
    tmp.file("src/bar.rs", "//\n");
    let tests = tmp.file("src/bar_tests.rs", &"//\n".repeat(250));
    assert_eq!(limit_for(&tests), MAX_FILE_LINES);
    assert_eq!(check_file(&tests), None);
    // A module directory is a source too: `<x>/` beside it (it holds `<x>/mod.rs`).
    tmp.file("src/baz/mod.rs", "//\n");
    assert_eq!(limit_for(&tmp.file("src/baz_tests.rs", "")), MAX_FILE_LINES);
    // The standalone `_test.rs` (singular) form still gets the tight cap.
    assert_eq!(limit_for(Path::new("src/bar_test.rs")), MAX_TEST_FILE_LINES);
}

#[test]
fn an_orphan_tests_file_gets_the_tight_cap() {
    // #960: the suffix alone once earned the source cap. With no `foo.rs` or `foo/`
    // beside it, `foo_tests.rs` is a standalone test file, and 151 lines fail.
    let tmp = Scratch::new("960-orphan");
    let orphan = tmp.file("src/foo_tests.rs", &"//\n".repeat(MAX_TEST_FILE_LINES + 1));
    tmp.file("src/foo_other.rs", "//\n");
    assert_eq!(limit_for(&orphan), MAX_TEST_FILE_LINES);
    let line = check_file(&orphan).expect("an orphan over the tight cap must fail");
    assert!(line.ends_with("151/150"), "{line}");
    // A bare `_tests.rs` has no source name at all.
    assert_eq!(
        limit_for(&tmp.file("src/_tests.rs", "")),
        MAX_TEST_FILE_LINES
    );
}

#[test]
fn full_scan_flags_an_oversized_integration_test() {
    // #535: the full scan once walked only `src/`, so `tests/` files over the
    // tight cap passed unseen. Pin that `tests/` is a root and that a scanned
    // file there over [`MAX_TEST_FILE_LINES`] is a violation.
    assert!(SCAN_ROOTS.contains(&"tests"));
    let root = std::env::temp_dir().join(format!("rusty-lint-535-{}", std::process::id()));
    let file = root.join("tests").join("too_long.rs");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "//\n".repeat(MAX_TEST_FILE_LINES + 1)).unwrap();
    let generated = root.join("tests").join("target").join("gen.rs");
    fs::create_dir_all(generated.parent().unwrap()).unwrap();
    fs::write(&generated, "//\n".repeat(MAX_TEST_FILE_LINES + 1)).unwrap();
    let found = scan_dir(&root.join("tests"));
    let flagged = found.iter().filter_map(|p| check_file(p)).count();
    fs::remove_dir_all(&root).ok();
    assert_eq!(found, vec![file]);
    assert_eq!(flagged, 1, "a tests/ file over the cap must fail the scan");
}

#[test]
fn full_scan_covers_tools_but_not_its_build_output() {
    // #541: `tools/` is a root, and the lint crate's own `tools/lint/target/`
    // (cargo build output) is never descended into.
    assert!(SCAN_ROOTS.contains(&"tools"));
    let root = std::env::temp_dir().join(format!("rusty-lint-541-{}", std::process::id()));
    let file = root
        .join("tools")
        .join("lint")
        .join("src")
        .join("too_long.rs");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "//\n".repeat(MAX_FILE_LINES + 1)).unwrap();
    let generated = root
        .join("tools")
        .join("lint")
        .join("target")
        .join("gen.rs");
    fs::create_dir_all(generated.parent().unwrap()).unwrap();
    fs::write(&generated, "//\n".repeat(MAX_FILE_LINES + 1)).unwrap();
    let found = scan_dir(&root.join("tools"));
    let flagged = found.iter().filter_map(|p| check_file(p)).count();
    fs::remove_dir_all(&root).ok();
    assert_eq!(found, vec![file]);
    assert_eq!(flagged, 1, "a tools/ file over the cap must fail the scan");
}

#[test]
fn normalize_strips_leading_dot_slash() {
    assert_eq!(normalize(Path::new("./src/main.rs")), "src/main.rs");
}

#[test]
fn python_tests_get_the_tight_cap_and_scripts_the_source_cap() {
    // #926: a `.py` test is any file under a `tests/` directory.
    let tests = ".github/scripts/tests/queue/test_taking.py";
    assert_eq!(limit_for(Path::new(tests)), MAX_TEST_FILE_LINES);
    let fixture = ".github/scripts/tests/queue/fixtures.py";
    assert_eq!(limit_for(Path::new(fixture)), MAX_TEST_FILE_LINES);
    let script = ".github/scripts/merging/queue/taking.py";
    assert_eq!(limit_for(Path::new(script)), MAX_FILE_LINES);
}

#[test]
fn a_python_file_over_the_cap_in_code_fails_and_says_lines_of_code() {
    // #926: `merge-queue.py` reached 1,074 lines because the gate never read `.py`.
    let tmp = Scratch::new("926-over");
    let path = tmp.file("over.py", &"x = 1\n".repeat(MAX_FILE_LINES + 1));
    let line = check_file(&path).expect("301 lines of code must fail");
    assert!(line.ends_with("301/300 (lines of code)"), "{line}");
}

#[test]
fn python_prose_does_not_count_against_the_cap() {
    // Docstrings and comments are free: a well-explained file is not a long one.
    let prose = format!(
        "\"\"\"A module.\n{}\"\"\"\n\n# {}\n",
        "Why it is shaped so.\n".repeat(400),
        "a comment\n# ".repeat(400)
    );
    let tmp = Scratch::new("926-prose");
    let path = tmp.file("prose.py", &(prose + &"x = 1\n".repeat(MAX_FILE_LINES)));
    assert_eq!(check_file(&path), None);
}

#[test]
fn the_full_scan_measures_github_scripts_but_not_compiled_python() {
    // #926: `.github` is a size root; `__pycache__` is build output, like `target/`.
    assert!(SIZE_ROOTS.contains(&".github"));
    let tmp = Scratch::new("926-scan");
    let script = tmp.file(".github/scripts/merge.py", "x = 1\n");
    tmp.file(".github/scripts/__pycache__/merge.py", "x = 1\n");
    tmp.file(".github/workflows/ci.yml", "name: ci\n");
    assert_eq!(measured_in(&tmp.0.join(".github")), vec![script]);
}

#[test]
fn the_rules_scan_still_reads_only_rust() {
    // The rules gate's checks are Rust's; a `.py` beside a `.rs` is not theirs.
    let tmp = Scratch::new("926-rules");
    let rust = tmp.file("tools/x/src/a.rs", "//\n");
    tmp.file("tools/x/a.py", "x = 1\n");
    assert_eq!(scan_dir(&tmp.0.join("tools")), vec![rust]);
}

#[test]
fn a_shader_or_script_over_the_cap_in_code_fails_and_says_lines_of_code() {
    // #1015: `shader.wgsl` and `input_field.lua` passed the cap unseen.
    let tmp = Scratch::new("1015-over");
    let shader = tmp.file("engine/shaders/s.wgsl", &"let a = 1;\n".repeat(301));
    let script = tmp.file("engine/scripts/s.lua", &"x = 1\n".repeat(301));
    for path in [shader, script] {
        let line = check_file(&path).expect("301 lines of code must fail");
        assert!(line.ends_with("301/300 (lines of code)"), "{line}");
    }
}

#[test]
fn shader_and_script_prose_does_not_count_against_the_cap() {
    let tmp = Scratch::new("1015-prose");
    let wgsl = format!(
        "/*\n{}*/\n// {}\n",
        "maths\n".repeat(400),
        "why\n// ".repeat(400)
    );
    let shader = tmp.file("s.wgsl", &(wgsl + &"let a = 1;\n".repeat(300)));
    assert_eq!(check_file(&shader), None);
    let lua = format!(
        "--[[\n{}]]\n-- {}\n",
        "why\n".repeat(400),
        "how\n-- ".repeat(400)
    );
    let script = tmp.file("s.lua", &(lua + &"x = 1\n".repeat(300)));
    assert_eq!(check_file(&script), None);
}

#[test]
fn the_full_scan_measures_the_engine_shaders_and_scripts() {
    // #1015: `engine/` is a size root; its fonts and models are not measured.
    assert!(SIZE_ROOTS.contains(&"engine"));
    let tmp = Scratch::new("1015-scan");
    let shader = tmp.file("engine/shaders/a.wgsl", "let a = 1;\n");
    let script = tmp.file("engine/scripts/b.lua", "x = 1\n");
    tmp.file("engine/models/m.obj", "v 0 0 0\n");
    let mut found = measured_in(&tmp.0.join("engine"));
    found.sort();
    assert_eq!(found, vec![script, shader]);
}

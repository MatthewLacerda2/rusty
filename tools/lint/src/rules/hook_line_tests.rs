use super::{check, File, RULE};
use crate::rules::test_code::test_lines;

const CRATES: &[&str] = &["dev", "lighting", "render"];

/// The lines `check` flags in the hook's file, over `files` beside it; every crate
/// depends on every other.
fn flagged(hook: &str, files: &[(&str, &str)]) -> Vec<(usize, String)> {
    let all: Vec<(&str, &str)> = std::iter::once(("crates/lighting/src/occlusion/x.rs", hook))
        .chain(files.iter().copied())
        .collect();
    let tests: Vec<Vec<bool>> = all.iter().map(|(p, s)| test_lines(p, s, false)).collect();
    let files: Vec<File> = all
        .iter()
        .zip(&tests)
        .map(|((path, source), test)| File { path, source, test })
        .collect();
    let crates: Vec<String> = CRATES.iter().map(ToString::to_string).collect();
    let found = check(&files, &crates, &|_, _| true);
    assert!(found.iter().all(|f| f.rule == RULE));
    found.into_iter().map(|f| (f.line, f.message)).collect()
}

const FREE: &str =
    "/// A test hook: `dev`'s cost tests time it.\n#[doc(hidden)]\npub fn probe() {}\n";

#[test]
fn a_hook_another_crate_calls_outside_tests_is_flagged() {
    for caller in [
        "fn f() {\n    rusty_lighting::occlusion::probe();\n}\n",
        "use rusty_lighting::occlusion::x::{probe, Grid};\nfn f() {\n    probe();\n}\n",
        "fn f() {\n    rusty::lighting::occlusion::x::probe();\n}\n",
    ] {
        let found = flagged(FREE, &[("crates/dev/src/a.rs", caller)]);
        assert_eq!(found.len(), 1, "{caller}");
        assert_eq!(found[0].0, 1);
        assert!(found[0].1.contains("crates/dev/src/a.rs:"), "{found:?}");
    }
    let method = "impl Bake {\n    /// A test hook.\n    pub fn sampler(&self) {}\n}\n";
    let caller = "use rusty_lighting::Bake;\nfn f(b: &Bake) {\n    Bake::sampler(b);\n}\n";
    assert_eq!(flagged(method, &[("crates/dev/src/a.rs", caller)]).len(), 1);
    let module = "//! Test hooks: the levels other crates bake.\npub fn level() {}\n";
    let caller = "use rusty_lighting::occlusion::x::level;\n";
    assert_eq!(flagged(module, &[("src/main.rs", caller)]).len(), 1);
    let wrapped = "use rusty_lighting::{\n    occlusion::x::probe,\n    Bake,\n};\nfn f() {\n    probe();\n}\n";
    let found = flagged(FREE, &[("crates/dev/src/a.rs", wrapped)]);
    assert_eq!(found.len(), 1, "an import rustfmt wrapped is read whole");
}

#[test]
fn a_call_to_the_caller_s_own_path_of_the_same_name_passes() {
    let hook = "/// A test hook: `dev`'s cost tests time it.\npub fn run() {}\n";
    let own = "use rusty_lighting::Bake;\nfn f() {\n    crate::occlusion::run();\n    occlusion::x::run();\n}\n";
    let dev_test = ("crates/dev/src/z_tests.rs", "fn t() { x::run(); }\n");
    let found = flagged(hook, &[("crates/dev/src/a.rs", own), dev_test]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_hook_only_tests_or_other_hooks_call_passes() {
    let in_tests = "fn f() {}\n#[cfg(test)]\nmod tests {\n    fn t() {\n        rusty_lighting::occlusion::probe();\n    }\n}\n";
    let other_hook = "//! Test hooks: a level built on the probe.\nfn f() {\n    rusty_lighting::occlusion::probe();\n}\n";
    for (path, caller) in [
        ("crates/dev/src/a.rs", in_tests),
        (
            "crates/dev/src/a_tests.rs",
            "fn t() { rusty_lighting::occlusion::probe(); }\n",
        ),
        (
            "crates/dev/tests/a.rs",
            "fn t() { rusty_lighting::occlusion::probe(); }\n",
        ),
        ("crates/dev/src/b.rs", other_hook),
        (
            "crates/dev/src/c.rs",
            "use rusty_dev::probe;\nfn f() { other::probe(); }\n",
        ),
        ("crates/dev/src/d.rs", "use rusty_lighting::probe::Grid;\n"),
        (
            "crates/dev/src/e.rs",
            "pub use rusty_lighting::occlusion::probe;\n",
        ),
        (
            "crates/lighting/src/f.rs",
            "fn f() { crate::occlusion::probe(); }\n",
        ),
    ] {
        let dev_test = ("crates/dev/src/z_tests.rs", "fn t() { x::probe(); }\n");
        let found = flagged(FREE, &[(path, caller), dev_test]);
        assert!(found.is_empty(), "{path}: {caller} {found:?}");
    }
    let cfg_test = "/// A test hook.\n#[cfg(test)]\npub fn probe() {}\n";
    let caller = (
        "crates/dev/src/a.rs",
        "fn f() { rusty_lighting::occlusion::probe(); }\n",
    );
    assert!(
        flagged(cfg_test, &[caller]).is_empty(),
        "no other crate sees it"
    );
}

#[test]
fn a_hook_line_naming_a_crate_that_never_calls_it_is_flagged() {
    let dev_test = ("crates/dev/src/a_tests.rs", "fn t() { x::probe(); }\n");
    assert!(flagged(FREE, &[dev_test]).is_empty());
    let found = flagged(
        FREE,
        &[("crates/render/src/a_tests.rs", "fn t() { x::probe(); }\n")],
    );
    assert_eq!(found.len(), 1);
    assert!(found[0]
        .1
        .contains("names dev, whose code never names `probe`"));
    let root = "/// A test hook for the root's tests.\npub fn probe() {}\n";
    assert_eq!(flagged(root, &[("src/main.rs", "fn main() {}\n")]).len(), 1);
    let tests = (
        "tests/a.rs",
        "fn t() { rusty::lighting::occlusion::probe(); }\n",
    );
    assert!(flagged(root, &[tests]).is_empty());
}

#[test]
fn what_a_hook_line_names_before_the_phrase_or_as_its_own_module_is_no_caller() {
    let before = "/// What `render` draws. A test hook.\npub fn probe() {}\n";
    assert!(flagged(before, &[]).is_empty());
    let own_module = "/// A test hook: `occlusion::bake`'s tests.\npub fn probe() {}\n";
    let own = ("crates/lighting/src/occlusion/bake.rs", "fn f() {}\n");
    assert!(flagged(own_module, &[own]).is_empty());
    let a_type = "/// A test hook: `dev`'s cost tests drive it.\npub struct Probe;\n";
    assert!(
        flagged(a_type, &[]).is_empty(),
        "a type is reached without its name"
    );
}

use super::{check_file, declares_gpu_test, render_path, scope, RULE};

fn check(path: &str, source: &str) -> Vec<usize> {
    let test = crate::rules::test_code::test_lines(path, source, false);
    let crates = ["render".to_string(), "lighting".to_string()];
    check_file(path, source, &test, &crates)
        .iter()
        .map(|f| f.line)
        .collect()
}

#[test]
fn flags_code_that_exists_only_without_dev_on_the_render_path() {
    let at = "crates/render/src/a.rs";
    assert_eq!(
        check(at, "#[cfg(not(feature = \"dev\"))]\nfn f() {}\n"),
        [1]
    );
    assert_eq!(
        check(at, "#[cfg(not(any(test, feature=\"dev\")))]\nfn f() {}\n"),
        [1]
    );
    assert_eq!(
        check(
            at,
            "#[cfg_attr(not(feature = \"dev\"), inline)]\nfn f() {}\n"
        ),
        [1]
    );
    assert_eq!(
        check(
            at,
            "const W: u32 = if cfg!(feature = \"dev\") { 1 } else { 0 };\n"
        ),
        [1]
    );
    assert_eq!(
        check(
            "crates/lighting/src/b/c.rs",
            "let s = cfg!(not(feature = \"dev\"));\n"
        ),
        [1]
    );
}

#[test]
fn flags_an_attribute_rustfmt_wrapped_over_lines() {
    let src =
        "fn a() {}\n#[cfg(all(\n    feature = \"x\",\n    not(feature = \"dev\")\n))]\nfn f() {}\n";
    assert_eq!(check("crates/render/src/a.rs", src), [2]);
    let attr = "#[cfg_attr(\n    not(feature = \"dev\"),\n    path = \"b.rs\"\n)]\nmod b;\n";
    assert_eq!(check("crates/render/src/a.rs", attr), [1]);
}

#[test]
fn flags_a_test_compiled_only_without_dev_but_not_a_tests_cfg_value() {
    let at = "crates/render/src/a.rs";
    let hidden = "#[cfg(test)]\nmod tests {\n    #[cfg(not(feature = \"dev\"))]\n    #[test]\n    fn gpu_x() {}\n}\n";
    assert_eq!(check(at, hidden), [3]);
    let read = "#[cfg(test)]\nmod tests {\n    #[test]\n    fn gpu_x() { let _ = cfg!(feature = \"dev\"); }\n}\n";
    assert!(check(at, read).is_empty(), "{:?}", check(at, read));
}

#[test]
fn passes_additions_other_crates_features_tests_and_comments() {
    let at = "crates/render/src/a.rs";
    let src = "#[cfg(feature = \"dev\")]\nfn hook() {}\n#[cfg(not(feature = \"editor\"))]\nfn g() {}\n// cfg!(feature = \"dev\") reads here\nlet s = \"cfg!(feature = \\\"dev\\\")\";\nconst W: bool = cfg!(any(feature = \"dev\", feature = \"editor\"));\n";
    assert!(check(at, src).is_empty(), "{:?}", check(at, src));
    let gated = "#[cfg(test)]\nmod tests {\n    fn f() -> bool { cfg!(feature = \"dev\") }\n}\n";
    assert!(check(at, gated).is_empty());
    let added = "#[cfg(all(\n    test,\n    feature = \"dev\"\n))]\nmod tests {}\n";
    assert!(check(at, added).is_empty());
    let off_path = "#[cfg(not(feature = \"dev\"))]\nfn f() {}\n";
    assert!(check("crates/shell/src/a.rs", off_path).is_empty());
    assert!(check("crates/render/tests/a.rs", off_path).is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src = "// lint: allow(gpu-default-only) only the window reads it\nconst W: bool = cfg!(feature = \"dev\");\n";
    let line = check("crates/render/src/a.rs", src)[0];
    assert!(crate::rules::allowed(src, line, RULE));
}

#[test]
fn the_render_path_is_render_and_what_its_dev_turns_on() {
    let repo = crate::repo::scaffold::Repo::new("gpu-default-only", false);
    repo.file(
        "crates/render/Cargo.toml",
        "[features]\ndev = [\"rusty-asset/dev\", \"rusty-scene?/dev\", \"glam/debug\"]\neditor = [\"rusty-core/editor\"]\n",
    );
    repo.file(
        "crates/scene/Cargo.toml",
        "[features]\ndev = [\n    \"rusty-asset/dev\",\n    \"rusty-core/dev\",\n]\n",
    );
    assert_eq!(render_path(&repo.0), ["render", "asset", "scene", "core"]);
    assert_eq!(
        scope(&repo.0, ["edit".to_string(), "core".to_string()]),
        ["render", "asset", "scene", "core", "edit"]
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let today = render_path(&root);
    assert!(
        today.len() > 2 && today.iter().any(|c| c == "lighting"),
        "{today:?}"
    );
}

#[test]
fn a_crate_with_an_in_crate_gpu_test_is_in_scope() {
    let src =
        "fn gpu_like() {}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn gpu_draws() {}\n}\n";
    let path = "crates/edit/src/a.rs";
    let test = crate::rules::test_code::test_lines(path, src, false);
    assert_eq!(declares_gpu_test(path, src, &test).as_deref(), Some("edit"));
    let production = "fn gpu_like() {}\n";
    let none = crate::rules::test_code::test_lines(path, production, false);
    assert_eq!(declares_gpu_test(path, production, &none), None);
    assert_eq!(
        declares_gpu_test("crates/edit/tests/a.rs", src, &test),
        None
    );
}

use super::test_shape::{check, GPU_ENTRY, INNER_CFG, USER_WORKSPACE};

fn rules(path: &str, src: &str) -> Vec<(usize, &'static str)> {
    check(path, src).iter().map(|f| (f.line, f.rule)).collect()
}

#[test]
fn an_inner_cfg_in_tests_is_flagged_and_a_declaration_cfg_passes() {
    let file = "//! A dev-only test.\n#![cfg(feature = \"dev\")]\nuse rusty::x;\n";
    assert_eq!(rules("tests/a.rs", file), [(2, INNER_CFG)]);
    let main = "#[cfg(feature = \"dev\")]\nmod a;\n#[cfg(unix)]\n#[test]\nfn b() {}\n";
    assert!(rules("tests/main.rs", main).is_empty());
}

#[test]
fn the_user_workspace_is_flagged_in_test_code_only() {
    let src = "fn cli() { scenario::run(&p, &o); Harness::in_user_workspace(o, b); }\n\
               #[cfg(test)]\nmod tests {\n    fn t() { scenario::run(&p, &o); }\n\
               fn u() { scenario::run_isolated(&p, &o); }\n}\n";
    assert_eq!(rules("src/dev/scenario.rs", src), [(4, USER_WORKSPACE)]);
    let it = "let h = Harness::in_user_workspace(dir, BOT);\n";
    assert_eq!(rules("tests/bot.rs", it), [(1, USER_WORKSPACE)]);
}

#[test]
fn a_direct_headless_renderer_is_flagged_outside_the_helper() {
    let call = "let r = pollster::block_on(Renderer::new_headless(64, 64));\n";
    assert_eq!(rules("src/render/a_tests.rs", call), [(1, GPU_ENTRY)]);
    assert_eq!(rules("tests/gpu/b.rs", call), [(1, GPU_ENTRY)]);
    assert!(rules("src/render/test_gpu.rs", call).is_empty());
    assert!(
        rules("src/dev/probe_bake.rs", call).is_empty(),
        "production"
    );
    let ok = "let Some(r) = headless_or_skip(64, 64) else { return };\n";
    assert!(rules("src/render/a_tests.rs", ok).is_empty());
}

#[test]
fn comments_and_other_roots_never_count() {
    let doc = "//! Uses `Renderer::new_headless(1, 1)` and `scenario::run(` in prose.\n";
    assert!(rules("tests/a.rs", doc).is_empty());
    let call = "#![cfg(x)]\nRenderer::new_headless(1, 1);\n";
    assert!(rules("tools/lint/src/a_tests.rs", call).is_empty());
}

#[test]
fn cfg_all_test_is_test_code_and_any_test_is_not() {
    let call = "    fn t() { scenario::run(&p, &o); }\n";
    let all = format!("#[cfg(all(test, feature = \"dev\"))]\nmod tests {{\n{call}}}\n");
    assert_eq!(rules("src/dev/a.rs", &all), [(3, USER_WORKSPACE)]);
    let any = all.replace("all(", "any(");
    assert!(rules("src/dev/a.rs", &any).is_empty());
}

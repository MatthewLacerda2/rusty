use super::{check, RULE};

/// The lines `check` flags in `src` at `path`.
fn lines(path: &str, src: &str) -> Vec<usize> {
    check(path, src).iter().map(|f| f.line).collect()
}

#[test]
fn flags_a_log_call_in_a_module_the_workers_reach() {
    let src = "fn f() {\n    log::warn!(\"[Asset] x\");\n}\n";
    for path in [
        "crates/asset/src/tangents.rs",
        "crates/asset/src/gltf/import.rs",
        "crates/asset/src/lightmap_uv/chart.rs",
        "crates/asset/src/lightmap_uv/tests/x.rs",
    ] {
        let found: Vec<_> = check(path, src).iter().map(|f| (f.line, f.rule)).collect();
        assert_eq!(found, [(2, RULE)], "{path}");
    }
    for call in ["warn!(\"x\");", "log::info!(\"x\");", "error!(\"x\");"] {
        assert_eq!(lines("crates/asset/src/clusters.rs", call), [1], "{call}");
    }
}

#[test]
fn passes_a_held_warning_and_modules_outside_the_list() {
    let held = "rusty_core::warn_held!(\"[Asset] x\");\n// log::warn! is not held\n";
    assert!(check("crates/asset/src/tangents.rs", held).is_empty());
    let warn = "log::warn!(\"[Asset] x\");\n";
    for path in [
        "crates/asset/src/gltf/material.rs",
        "crates/asset/src/cook/mod.rs",
        "crates/asset/src/tangents_more.rs",
        "crates/asset/src/lightmap_uv.rs.bak",
        "crates/render/src/tangents.rs",
    ] {
        assert!(check(path, warn).is_empty(), "{path}");
    }
    assert!(check("crates/asset/src/obj_import.rs", "let dialog::warn = x;\n").is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src = "// lint: allow(held-warn) said before any thread starts\nlog::warn!(\"x\");\n";
    let found = check("crates/asset/src/obj_import.rs", src);
    assert!(crate::rules::allowed(src, found[0].line, RULE));
}

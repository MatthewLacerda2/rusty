use super::check;
use super::graph::Edge;
use super::scan::refs;
use super::table::{Layer, LAYERS};

fn names(code: &str) -> Vec<String> {
    refs(code).into_iter().map(|r| r.module).collect()
}

#[test]
fn finds_sibling_crate_paths() {
    let code = "use crate::paths::x;\nuse rusty_core::{a, b};\nfn f() { rusty_time::Time::new(); }";
    assert_eq!(names(code), ["core", "time"]);
    assert!(names("use my_rusty_core::x;\nlet rusty_x = 1;").is_empty());
    assert!(names("use rusty::crate_like::X; // rusty_render::X").is_empty());
}

#[test]
fn skips_test_items() {
    let code = "#[cfg(test)]\nmod tests {\n    use rusty_render::X;\n}\nuse rusty_core::Y;";
    assert_eq!(names(code), ["core"]);
    assert_eq!(
        names("#[cfg(test)] use rusty_render::X;\nuse rusty_ui::Z;"),
        ["ui"]
    );
}

#[test]
fn an_any_test_import_is_checked_and_a_wrapped_all_test_one_is_skipped() {
    let any = "#[cfg(any(test, feature = \"x\"))]\nuse rusty_render::X;";
    assert_eq!(names(any), ["render"]);
    let all =
        "#[cfg(all(\n    feature = \"dev\",\n    test\n))]\nuse rusty_render::X;\nuse rusty_ui::Z;";
    assert_eq!(names(all), ["ui"]);
}

#[test]
fn a_gated_fn_with_a_wrapped_signature_is_skipped_to_its_closing_brace() {
    let code =
        "#[cfg(test)]\nfn f<T>(t: T) -> Result<\n    A,\n    B,\n>\nwhere\n    T: X,\n{\n    \
                use rusty_render::X;\n}\nuse rusty_core::Y;";
    assert_eq!(names(code), ["core"]);
}

#[test]
fn a_comment_after_a_raw_string_closing_on_a_later_line_is_cut() {
    let code = "let s = r#\"\n(\n\"#; // see rusty_render::X\nuse rusty_core::Y;";
    assert_eq!(names(code), ["core"]);
}

fn edge(from: &str, to: &str) -> Edge {
    Edge {
        from: from.into(),
        to: to.into(),
        at: "crates/x/src/lib.rs:1".into(),
    }
}

const ROWS: &[Layer] = &[
    Layer {
        module: "core",
        sim: true,
        deps: &[],
        crates: &[],
    },
    Layer {
        module: "scene",
        sim: true,
        deps: &["core"],
        crates: &[],
    },
    Layer {
        module: "render",
        sim: false,
        deps: &["scene"],
        crates: &[],
    },
];

fn run(rows: &[Layer], edges: &[Edge]) -> Vec<String> {
    let modules: Vec<String> = rows.iter().map(|l| l.module.to_string()).collect();
    check::all(rows, &modules, edges)
}

#[test]
fn exact_table_passes() {
    assert!(run(ROWS, &[edge("scene", "core"), edge("render", "scene")]).is_empty());
}

#[test]
fn flags_a_stale_dep_and_a_crate_without_its_row() {
    let v = run(ROWS, &[edge("scene", "core")]);
    assert!(
        v.iter().any(|m| m.starts_with("STALE_DEP render → scene")),
        "{v:?}"
    );
    let modules = ["core".to_string(), "scene".into(), "ui".into()];
    let v = check::all(
        ROWS,
        &modules,
        &[edge("scene", "core"), edge("render", "scene")],
    );
    let has = |p: &str| v.iter().any(|m| m.starts_with(p));
    assert!(has("UNDECLARED_MODULE crates/ui"), "{v:?}");
    assert!(has("MISSING_MODULE render"), "{v:?}");
}

#[test]
fn flags_cycles_and_sim_leaks() {
    let rows = [
        Layer {
            module: "core",
            sim: true,
            deps: &["scene"],
            crates: &[],
        },
        Layer {
            module: "scene",
            sim: true,
            deps: &["render"],
            crates: &[],
        },
        Layer {
            module: "render",
            sim: false,
            deps: &[],
            crates: &[],
        },
    ];
    let v = run(&rows, &[edge("core", "scene"), edge("scene", "render")]);
    assert!(
        v.iter().any(|m| m.starts_with("CYCLE core → scene")),
        "{v:?}"
    );
    assert!(
        v.iter().any(|m| m.starts_with("SIM_LEAK scene → render")),
        "{v:?}"
    );
}

#[test]
fn real_table_is_well_formed() {
    let modules: Vec<String> = LAYERS.iter().map(|l| l.module.to_string()).collect();
    let edges: Vec<Edge> = LAYERS
        .iter()
        .flat_map(|l| l.deps.iter().map(|d| edge(l.module, d)))
        .collect();
    let v = check::all(LAYERS, &modules, &edges);
    assert!(v.is_empty(), "{v:?}");
}

#[test]
fn sim_set_is_pinned() {
    // Dropping a module out of the sim set must be a deliberate, reviewed edit (#723).
    let mut dirs = super::sim_dirs();
    dirs.sort();
    let want = [
        "ai",
        "app",
        "asset",
        "audio",
        "components",
        "core",
        "ecs",
        "navigation",
        "physics",
        "procgen",
        "scene",
        "script",
        "shadergen",
        "time",
        "ui",
    ];
    assert_eq!(dirs, want.map(|m| format!("src/{m}")));
}

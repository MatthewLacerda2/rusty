use super::check;
use super::graph::Edge;
use super::scan::{is_test_cfg, refs};
use super::table::{Exception, Layer, EXCEPTIONS, LAYERS};

fn names(code: &str) -> Vec<String> {
    refs(code).into_iter().map(|r| r.module).collect()
}

#[test]
fn finds_paths_and_groups() {
    assert_eq!(names("use crate::scene::Scene;"), ["scene"]);
    assert_eq!(names("let x = crate::core::f();"), ["core"]);
    assert_eq!(
        names("use crate::{scene::Scene, ui::{A, B}};"),
        ["scene", "ui"]
    );
    assert_eq!(
        names("use crate::{\n    scene::Scene,\n    ui::{A, B},\n};"),
        ["scene", "ui"]
    );
    assert!(names("use rusty::crate_like::X; // crate::render").is_empty());
}

#[test]
fn skips_test_items() {
    let code = "#[cfg(test)]\nmod tests {\n    use crate::render::X;\n}\nuse crate::core::Y;";
    assert_eq!(names(code), ["core"]);
    assert_eq!(
        names("#[cfg(test)] use crate::render::X;\nuse crate::ui::Z;"),
        ["ui"]
    );
    assert!(is_test_cfg("#[cfg(all(test, feature = \"dev\"))]"));
    assert!(!is_test_cfg("#[cfg(not(test))]"));
    assert!(!is_test_cfg("#[cfg(feature = \"testing\")]"));
}

fn edge(from: &str, to: &str) -> Edge {
    Edge {
        from: from.into(),
        to: to.into(),
        at: "src/x.rs:1".into(),
    }
}

const ROWS: &[Layer] = &[
    Layer {
        module: "core",
        sim: true,
        deps: &[],
    },
    Layer {
        module: "scene",
        sim: true,
        deps: &["core"],
    },
    Layer {
        module: "render",
        sim: false,
        deps: &["scene"],
    },
];

fn run(rows: &[Layer], edges: &[Edge]) -> Vec<String> {
    let modules: Vec<String> = rows.iter().map(|l| l.module.to_string()).collect();
    check::all(rows, &[], &[], &modules, edges)
}

#[test]
fn exact_table_passes() {
    assert!(run(ROWS, &[edge("scene", "core"), edge("render", "scene")]).is_empty());
}

#[test]
fn flags_undeclared_and_stale() {
    let v = run(ROWS, &[edge("scene", "core"), edge("scene", "render")]);
    assert!(v.iter().any(|m| m.starts_with("UNDECLARED_DEP")), "{v:?}");
    assert!(
        v.iter().any(|m| m.starts_with("STALE_DEP render → scene")),
        "{v:?}"
    );
}

#[test]
fn flags_cycles_and_sim_leaks() {
    let rows = [
        Layer {
            module: "core",
            sim: true,
            deps: &["scene"],
        },
        Layer {
            module: "scene",
            sim: true,
            deps: &["render"],
        },
        Layer {
            module: "render",
            sim: false,
            deps: &[],
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
fn exceptions_allow_until_fixed() {
    let x = [Exception {
        from: "scene",
        to: "render",
        issue: 1,
    }];
    let modules: Vec<String> = ROWS.iter().map(|l| l.module.to_string()).collect();
    let live = [
        edge("scene", "core"),
        edge("render", "scene"),
        edge("scene", "render"),
    ];
    assert!(check::all(ROWS, &[], &x, &modules, &live).is_empty());
    let v = check::all(ROWS, &[], &x, &modules, &live[..2]);
    assert!(v.iter().any(|m| m.starts_with("STALE_EXCEPTION")), "{v:?}");
}

#[test]
fn real_table_is_well_formed() {
    let modules: Vec<String> = LAYERS.iter().map(|l| l.module.to_string()).collect();
    let edges: Vec<Edge> = LAYERS
        .iter()
        .flat_map(|l| l.deps.iter().map(|d| edge(l.module, d)))
        .chain(EXCEPTIONS.iter().map(|x| edge(x.from, x.to)))
        .collect();
    let v = check::all(LAYERS, super::table::PEERS, EXCEPTIONS, &modules, &edges);
    assert!(v.is_empty(), "{v:?}");
    assert!(
        EXCEPTIONS.iter().all(|x| x.issue > 0),
        "every exception names its issue"
    );
}

#[test]
fn sim_set_is_pinned() {
    // Dropping a module out of the sim set must be a deliberate, reviewed edit (#723).
    let mut dirs = super::sim_dirs();
    dirs.sort();
    let want = [
        "api",
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
        "scripting",
        "shadergen",
        "time",
        "ui",
    ];
    assert_eq!(dirs, want.map(|m| format!("src/{m}")));
}

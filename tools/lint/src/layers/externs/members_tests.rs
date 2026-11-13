use super::{dev_stray, stray};
use crate::layers::externs::policy::POLICY;
use crate::layers::table::LAYERS;

const MANIFEST: &str = "[package]\nname = \"rusty-components\"\n\n[dependencies]\n\
    rusty-asset = { workspace = true }\nrusty-core = { workspace = true }\n\
    rusty-procgen = { workspace = true }\nbytemuck = { workspace = true }\n\
    glam = { workspace = true }\nserde = { workspace = true }\n\n\
    [dev-dependencies]\nrusty-test-support = { workspace = true }\nimage = { workspace = true }\n";

#[test]
fn a_member_naming_only_its_row_passes() {
    assert!(stray("components", MANIFEST, LAYERS, &POLICY).is_empty());
}

#[test]
fn a_stray_crate_or_sibling_is_flagged_by_name() {
    let text = MANIFEST.replace(
        "glam = { workspace = true }\n",
        "glam = { workspace = true }\nimage = { workspace = true }\nrusty-render = { workspace = true }\n",
    );
    let found = stray("components", &text, LAYERS, &POLICY).join("\n");
    for name in ["→ image —", "→ rusty-render —"] {
        assert!(found.contains(name), "not flagged: {name}\n{found}");
    }
    assert_eq!(found.lines().count(), 2, "{found}");
}

#[test]
fn a_member_that_is_no_module_of_the_table_is_left_alone() {
    let text = "[dependencies]\ntempfile = { workspace = true }\n";
    assert!(stray("test-support", text, LAYERS, &POLICY).is_empty());
}

#[test]
fn a_dev_dependency_on_a_module_below_or_on_test_support_passes() {
    let text = "[dev-dependencies]\nrusty-test-support = { workspace = true }\n\
        rusty-asset = { workspace = true }\nproptest = { workspace = true }\n";
    assert!(dev_stray("physics", text, LAYERS).is_empty());
}

#[test]
fn a_dev_dependency_on_itself_a_module_above_or_one_naming_it_back_is_flagged() {
    let text = "[dev-dependencies]\nrusty-render = { workspace = true }\n\
        rusty-physics = { workspace = true }\n\
        [target.'cfg(unix)'.dev-dependencies]\nrusty-navigation = { workspace = true }\n";
    let found = dev_stray("physics", text, LAYERS).join("\n");
    for name in [
        "→ rusty-render —",
        "→ rusty-physics —",
        "→ rusty-navigation —",
    ] {
        assert!(found.contains(name), "not flagged: {name}\n{found}");
    }
    assert_eq!(found.lines().count(), 3, "{found}");
}

#[test]
fn a_module_below_that_reaches_back_through_its_deps_is_flagged() {
    use crate::layers::table::Layer;
    let rows = [
        Layer {
            module: "a",
            sim: true,
            deps: &["b"],
            crates: &[],
        },
        Layer {
            module: "b",
            sim: true,
            deps: &[],
            crates: &[],
        },
    ];
    let text = "[dev-dependencies]\nrusty-a = { workspace = true }\n";
    assert_eq!(dev_stray("b", text, &rows).len(), 1);
}

#[test]
fn a_dev_dependency_in_a_one_crate_header_is_flagged() {
    let text = "[dev-dependencies.rusty-render]\nworkspace = true\n";
    assert_eq!(dev_stray("physics", text, LAYERS).len(), 1);
}

#[test]
fn a_stray_crate_under_a_bare_target_triple_is_flagged() {
    let text = "[target.aarch64-apple-darwin.dependencies]\nwgpu = { workspace = true }\n\
        [target.aarch64-apple-darwin.dev-dependencies]\nrusty-render = { workspace = true }\n";
    assert_eq!(stray("components", text, LAYERS, &POLICY).len(), 1);
    assert_eq!(dev_stray("physics", text, LAYERS).len(), 1);
}

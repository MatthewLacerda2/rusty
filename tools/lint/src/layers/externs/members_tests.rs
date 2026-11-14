use super::{dev_stray, display, stray, support};
use crate::layers::externs::policy::POLICY;
use crate::layers::table::{Layer, LAYERS};

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
fn a_dev_dependency_on_itself_or_a_module_above_is_flagged() {
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
fn a_dev_dependency_in_a_one_crate_header_is_flagged() {
    let text = "[dev-dependencies.rusty-render]\nworkspace = true\n";
    assert_eq!(dev_stray("physics", text, LAYERS).len(), 1);
}

#[test]
fn a_stray_crate_under_a_bare_target_triple_is_flagged() {
    let text = "[target.aarch64-apple-darwin.dependencies]\nwgpu = { workspace = true }\n\
        [target.aarch64-apple-darwin.dev-dependencies]\nrusty-render = { workspace = true }\n";
    let stray_found = stray("components", text, LAYERS, &POLICY);
    let strays = stray_found.iter().filter(|m| m.starts_with("STRAY_CRATE"));
    assert_eq!(strays.count(), 1, "{stray_found:?}");
    assert_eq!(dev_stray("physics", text, LAYERS).len(), 1);
}

#[test]
fn a_crate_the_row_names_but_the_manifest_lacks_is_flagged() {
    let text = MANIFEST
        .replace("rusty-procgen = { workspace = true }\n", "")
        .replace("bytemuck = { workspace = true }\n", "");
    let found = stray("components", &text, LAYERS, &POLICY).join("\n");
    for name in ["MISSING_CRATE", "→ rusty-procgen —", "→ bytemuck —"] {
        assert!(found.contains(name), "not flagged: {name}\n{found}");
    }
    assert_eq!(found.lines().count(), 2, "{found}");
}

#[test]
fn a_sim_member_naming_a_display_crate_even_in_tests_is_flagged() {
    let text = "[dependencies]\nwgpu = { workspace = true }\n\
        [dev-dependencies]\negui_extras = { workspace = true }\n\
        proptest = { workspace = true }\nnaga = { workspace = true }\n";
    let found = display("physics", text, LAYERS, &POLICY).join("\n");
    for name in ["→ wgpu —", "→ egui-extras —"] {
        assert!(found.contains(name), "not flagged: {name}\n{found}");
    }
    assert_eq!(found.lines().count(), 2, "{found}");
    assert!(display("render", text, LAYERS, &POLICY).is_empty());
}

#[test]
fn a_sim_member_testing_against_a_platform_sibling_below_it_is_flagged() {
    let row = |module, sim| Layer {
        module,
        sim,
        deps: &[],
        crates: &[],
    };
    let rows = [
        row("logging", false),
        row("core", true),
        row("physics", true),
    ];
    let text = "[dev-dependencies]\nrusty-logging = { workspace = true }\n\
        rusty-core = { workspace = true }\nrusty-test-support = { workspace = true }\n";
    let found = display("physics", text, &rows, &POLICY);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("PLATFORM_DEV_CRATE crates/physics/Cargo.toml → rusty-logging"));
    // The real table's own platform rows sit above every sim row: a dev-dependency
    // there is `STRAY_DEV_CRATE`'s, so one line never gets two findings.
    let up = "[dev-dependencies]\nrusty-render = { workspace = true }\n";
    assert!(display("physics", up, LAYERS, &POLICY).is_empty());
    assert_eq!(dev_stray("physics", up, LAYERS).len(), 1);
}

#[test]
fn test_support_naming_a_sibling_or_a_display_crate_is_flagged() {
    let text =
        "[dependencies]\nimage = { workspace = true }\nrusty-render = { workspace = true }\n\
        egui = { workspace = true }\n[dev-dependencies]\nrusty-core = { workspace = true }\n";
    let found = support("test-support", text).join("\n");
    for name in ["→ rusty-render —", "→ egui —", "→ rusty-core —"] {
        assert!(found.contains(name), "not flagged: {name}\n{found}");
    }
    assert_eq!(found.lines().count(), 3, "{found}");
    let plain = "[dependencies]\nimage = { workspace = true }\nlog = { workspace = true }\n";
    assert!(support("test-support", plain).is_empty());
    // A module of the table is its row's business, not this check's.
    assert!(support("physics", text).is_empty());
}

#[test]
fn display_names_a_crate_family_not_a_prefix() {
    for k in ["egui", "egui_wgpu", "egui_extras", "wgpu", "wgpu_types"] {
        assert!(POLICY.is_display(k), "{k}");
    }
    for k in ["eguilike", "naga", "rusty_render"] {
        assert!(!POLICY.is_display(k), "{k}");
    }
}

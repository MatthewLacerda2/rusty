use super::stray;
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

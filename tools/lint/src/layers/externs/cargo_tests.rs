use super::{dependencies, dev_keys, keys};

#[test]
fn reads_the_dependency_sections_only() {
    let manifest = "[package]\nname = \"x\"\n\n[dependencies]\nwgpu = \"1\"\n\
        egui-wgpu = { version = \"1\", optional = true } # a comment = x\n\
        zimmer = { package = \"scorsese-zimmer\", git = \"u\" }\n\n\
        [target.'cfg(target_os = \"linux\")'.dependencies]\nsmithay-clipboard = \"0.7\"\n\n\
        [dev-dependencies]\nproptest = \"1\"\n";
    assert_eq!(
        dependencies(manifest),
        ["egui_wgpu", "smithay_clipboard", "wgpu", "zimmer"]
    );
}

#[test]
fn a_member_manifest_names_its_crates_but_not_the_workspaces_own() {
    let manifest = "[dependencies]\nglam = { workspace = true }\nlog.workspace = true\n\
        rusty-core = { workspace = true }\n";
    assert_eq!(dependencies(manifest), ["glam", "log"]);
}

#[test]
fn a_one_crate_header_names_its_crate() {
    let text = "[dependencies.serde]\nversion = \"1\"\n\n[dev-dependencies.rusty-render]\n\
        workspace = true\n[target.'cfg(target_os = \"macos\")'.dev-dependencies.objc2]\nworkspace = true\n";
    assert_eq!(keys(text), ["serde"]);
    assert_eq!(dev_keys(text), ["objc2", "rusty_render"]);
}

#[test]
fn a_bare_target_triple_is_read() {
    let text = "[target.aarch64-apple-darwin.dependencies]\nobjc2 = 1\n\
        [target.x86_64-unknown-linux-gnu.dev-dependencies.wgpu]\nworkspace = true\n";
    assert_eq!(keys(text), ["objc2"]);
    assert_eq!(dev_keys(text), ["wgpu"]);
}

#[test]
fn a_header_whose_name_is_no_crate_reads_nothing() {
    let text = "[dependencies.\"1bad\"]\nversion = \"1\"\nfeatures = []\n";
    assert!(keys(text).is_empty());
}

#[test]
fn a_table_with_a_lookalike_name_is_not_read() {
    let text = "[dependencies-extra]\nfoo = 1\n[package.metadata.dependencies]\nbar = 1\n";
    assert!(keys(text).is_empty());
}

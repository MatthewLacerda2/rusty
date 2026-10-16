use super::cargo::dependencies;
use super::check;
use super::find::crate_refs;
use super::policy::{Confined, CrateException, Policy, POLICY};
use super::Use;
use crate::layers::table::{Layer, LAYERS};
use std::collections::BTreeSet;

fn found(code: &str) -> Vec<String> {
    let crates: BTreeSet<String> = ["wgpu", "image", "mlua", "glam"].map(String::from).into();
    crate_refs(code, &crates)
        .into_iter()
        .map(|r| r.krate)
        .collect()
}

#[test]
fn finds_use_paths_and_inline_paths() {
    assert_eq!(found("use wgpu::Device;"), ["wgpu"]);
    assert_eq!(found("pub(crate) use mlua::{Lua, Table};"), ["mlua"]);
    assert_eq!(found("use {glam::Vec3, wgpu::Queue};"), ["glam", "wgpu"]);
    assert_eq!(found("let d: wgpu::Device = x;"), ["wgpu"]);
    assert_eq!(found("fn f(v: Vec<image::RgbaImage>) {}"), ["image"]);
    assert_eq!(found("use mlua::{\n    Lua,\n    Table,\n};"), ["mlua"]);
}

#[test]
fn ignores_what_is_not_the_crate() {
    // A deeper segment, a nested group entry, a comment, test code.
    assert!(found("let x = crate::render::wgpu::f();").is_empty());
    assert!(found("use crate::scene::{image, wgpu::X};").is_empty());
    assert!(found("let a = 1; // wgpu::Device").is_empty());
    assert!(found("#[cfg(test)]\nmod t {\n    use wgpu::X;\n}").is_empty());
}

#[test]
fn a_local_item_shadows_the_crate() {
    assert!(found("pub mod image;\nfn f() { image::draw(); }").is_empty());
    assert!(found("use super::image;\nfn f() { image::draw(); }").is_empty());
    let aliased = "use crate::scene::authoring::{audio, image as img};\nfn f() { image::open(); }";
    assert_eq!(found(aliased), ["image"]);
}

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

const ROWS: &[Layer] = &[
    Layer {
        module: "scene",
        sim: true,
        deps: &[],
        crates: &[],
    },
    Layer {
        module: "render",
        sim: false,
        deps: &[],
        crates: &["wgpu"],
    },
];

const TIGHT: Policy = Policy {
    common: &["glam"],
    unscoped: &[("env_logger", "binaries only")],
    confined: &[Confined {
        krate: "wgpu",
        within: &["src/render/gpu"],
    }],
    exceptions: &[],
};

fn deps() -> Vec<String> {
    ["env_logger", "glam", "wgpu"].map(String::from).to_vec()
}

fn at(module: &str, krate: &str, file: &str) -> Use {
    Use {
        module: module.into(),
        krate: krate.into(),
        at: format!("{file}:1"),
    }
}

#[test]
fn exact_table_passes() {
    let uses = [
        at("render", "wgpu", "src/render/gpu/x.rs"),
        at("scene", "glam", "src/scene/x.rs"),
    ];
    let v = check::all(ROWS, &TIGHT, &deps(), &uses);
    assert!(v.is_empty(), "{v:?}");
}

#[test]
fn flags_undeclared_confined_and_stale() {
    let uses = [
        at("scene", "wgpu", "src/scene/x.rs"),
        at("scene", "env_logger", "src/scene/x.rs"),
    ];
    let v = check::all(ROWS, &TIGHT, &deps(), &uses);
    let has = |p: &str| v.iter().any(|m| m.starts_with(p));
    assert!(
        has("UNDECLARED_CRATE src/scene/x.rs:1 `scene → wgpu`"),
        "{v:?}"
    );
    assert!(has(
        "UNDECLARED_CRATE src/scene/x.rs:1 `scene → env_logger`"
    ));
    assert!(has("CONFINED_CRATE src/scene/x.rs:1 `wgpu`"), "{v:?}");
    assert!(has("STALE_CRATE render → wgpu"), "{v:?}");
    assert!(has("STALE_CRATE common → glam"), "{v:?}");
}

#[test]
fn flags_a_table_out_of_step_with_cargo() {
    let more = ["env_logger", "glam", "kira", "wgpu"].map(String::from);
    let rows = [Layer {
        module: "render",
        sim: false,
        deps: &[],
        crates: &["wgpu", "glam", "winit"],
    }];
    let uses = [at("render", "wgpu", "src/render/gpu/x.rs")];
    let v = check::all(&rows, &TIGHT, &more, &uses);
    let has = |p: &str| v.iter().any(|m| m.starts_with(p));
    assert!(has("UNCLAIMED_CRATE kira"), "{v:?}");
    assert!(has("UNKNOWN_CRATE winit"), "{v:?}");
    assert!(has("REDUNDANT_CRATE render → glam"), "{v:?}");
}

#[test]
fn exceptions_allow_until_fixed() {
    let policy = Policy {
        exceptions: &[CrateException {
            module: "scene",
            krate: "wgpu",
            issue: 1,
        }],
        confined: &[],
        ..TIGHT
    };
    let mut uses = vec![
        at("render", "wgpu", "src/render/gpu/x.rs"),
        at("scene", "glam", "src/scene/x.rs"),
        at("scene", "wgpu", "src/scene/x.rs"),
    ];
    assert!(check::all(ROWS, &policy, &deps(), &uses).is_empty());
    uses.pop();
    let v = check::all(ROWS, &policy, &deps(), &uses);
    assert!(v
        .iter()
        .any(|m| m.starts_with("STALE_EXCEPTION scene → wgpu")));
}

#[test]
fn real_table_accounts_for_every_dependency() {
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml");
    let deps = dependencies(&std::fs::read_to_string(manifest).expect("rusty's Cargo.toml"));
    let v = check::all(LAYERS, &POLICY, &deps, &[]);
    let shape: Vec<_> = v.iter().filter(|m| !m.starts_with("STALE")).collect();
    assert!(shape.is_empty(), "{shape:?}");
    assert!(POLICY.exceptions.iter().all(|x| x.issue > 0));
}

//! A baked shader re-bakes from its recipe when the engine's shaders change (#1638),
//! against a private copy of the engine shaders so a test can change one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{fingerprint, recipe_path, refresh_against, Refresh};
use crate::shadergen::compose::MODULES;
use crate::shadergen::recipe::{BlockSel, PassKind, ShaderRecipe};
use crate::shadergen::{bake_recipe, validate::validate};

/// A fresh `<tag>` dir holding a copy of the engine shaders a surface bake reads.
fn engine_copy(tag: &str) -> (PathBuf, String) {
    let root = crate::test_temp::dir().join(format!("rusty_1638_{tag}"));
    std::fs::remove_dir_all(&root).ok();
    let engine = root.join("engine");
    for name in MODULES.iter().chain(&["shader.wgsl"]) {
        let to = engine.join(name);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(format!("engine/shaders/{name}"), to).unwrap();
    }
    let engine = engine.to_str().unwrap().to_owned();
    (root, engine)
}

fn rim() -> ShaderRecipe {
    ShaderRecipe {
        pass: PassKind::Surface,
        name: "rim".into(),
        blocks: vec![BlockSel {
            id: "fresnel_rim".into(),
            params: BTreeMap::new(),
        }],
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn a_changed_engine_module_re_bakes_the_module_once() {
    let (root, engine) = engine_copy("module");
    let out = root.join("out");
    let wgsl = PathBuf::from(bake_recipe(&rim(), &engine, out.to_str().unwrap()).unwrap());
    assert!(recipe_path(&wgsl).is_file(), "the bake keeps its recipe");
    assert_eq!(refresh_against(&wgsl, &engine), Refresh::Current);

    let before = read(&wgsl);
    let common = Path::new(&engine).join("common.wgsl");
    std::fs::write(&common, read(&common) + "\n// a changed engine module\n").unwrap();
    assert_eq!(refresh_against(&wgsl, &engine), Refresh::Rebaked);
    let after = read(&wgsl);
    assert_ne!(
        after.lines().next(),
        before.lines().next(),
        "new fingerprint"
    );
    validate(&engine, &after).expect("the re-baked module composes");
    assert_eq!(refresh_against(&wgsl, &engine), Refresh::Current, "once");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_module_that_no_longer_composes_is_re_baked_and_composes_again() {
    // What #1616 did to every baked shader: the module imports a binding gone since.
    let (root, engine) = engine_copy("stranded");
    let out = root.join("out");
    let wgsl = PathBuf::from(bake_recipe(&rim(), &engine, out.to_str().unwrap()).unwrap());
    let stranded = "// rusty engine fingerprint: 0000000000000000\n\
        #import forward::bindings::{entity}\n\
    @fragment fn fs_main() -> @location(0) vec4<f32> { return entity.color_tint; }\n";
    std::fs::write(&wgsl, stranded).unwrap();
    assert!(validate(&engine, stranded).is_err());
    assert_eq!(refresh_against(&wgsl, &engine), Refresh::Rebaked);
    validate(&engine, &read(&wgsl)).expect("the re-baked module composes");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_module_without_a_recipe_is_left_alone() {
    let (root, engine) = engine_copy("bare");
    let wgsl = root.join("bare.wgsl");
    let text = "@fragment fn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }\n";
    std::fs::write(&wgsl, text).unwrap();
    assert_eq!(refresh_against(&wgsl, &engine), Refresh::Unbaked);
    assert_eq!(read(&wgsl), text, "untouched");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_recipe_that_cannot_be_re_baked_leaves_the_module_and_says_why() {
    let (root, engine) = engine_copy("broken");
    let wgsl = root.join("odd.wgsl");
    std::fs::write(&wgsl, "// old module\n").unwrap();
    for (recipe, why) in [
        (r#"{"pass":"surface","name":"other"}"#, "named \"other\""),
        (
            r#"{"pass":"surface","name":"odd","blocks":[{"id":"gone"}]}"#,
            "gone",
        ),
        ("{ truncated", "recipe.json"),
    ] {
        std::fs::write(recipe_path(&wgsl), recipe).unwrap();
        let Refresh::Failed(e) = refresh_against(&wgsl, &engine) else {
            panic!("{recipe} re-baked");
        };
        assert!(e.contains(why), "{e}");
        assert_eq!(read(&wgsl), "// old module\n", "{recipe}: untouched");
    }
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn the_fingerprint_moves_with_the_module_and_ignores_line_endings() {
    let (root, engine) = engine_copy("hash");
    let a = fingerprint("fn a() {}\n", &engine).unwrap();
    assert_eq!(a, fingerprint("fn a() {}\r\n", &engine).unwrap());
    assert_ne!(a, fingerprint("fn b() {}\n", &engine).unwrap());
    assert!(fingerprint("", "no/such/engine").is_err());
    std::fs::remove_dir_all(&root).ok();
}

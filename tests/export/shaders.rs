//! A baked shader stale against the engine's shaders is re-baked from its recipe
//! by the export, so the game ships modules current with its engine and the
//! player composes none (#1638).

use std::collections::BTreeMap;

use rusty::shadergen::rebake::{refresh, Refresh};
use rusty::shadergen::{bake_recipe, engine_shader_dir, BlockSel, PassKind, ShaderRecipe};

use super::{export, project, tree};

/// A one-block surface recipe, as `Shader.Bake` writes one.
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

#[test]
fn an_export_ships_a_stale_baked_shader_re_baked() {
    let game = project(&[], "");
    let shaders = game.join("assets").join("shaders");
    let wgsl = bake_recipe(&rim(), engine_shader_dir(), shaders.to_str().unwrap()).unwrap();
    let current = std::fs::read_to_string(&wgsl).unwrap();
    // Baked against other engine shaders: its recorded fingerprint no longer matches.
    let (_, body) = current.split_once('\n').unwrap();
    let stale = format!("// rusty engine fingerprint: 0000000000000000\n{body}");
    std::fs::write(&wgsl, &stale).unwrap();

    let out = crate::temp::dir().join("out");
    let exported = export(&game, &out);
    assert!(exported.status.success(), "{exported:?}");
    let shipped = tree(&out);
    let module = shipped
        .get("assets/shaders/rim.wgsl")
        .expect("the module ships");
    assert_eq!(String::from_utf8_lossy(module), current, "re-baked");
    for kept in ["rim.params.json", "rim.recipe.json"] {
        let kept = format!("assets/shaders/{kept}");
        assert!(shipped.contains_key(&kept), "{kept} ships");
    }
    let shipped_wgsl = out.join("assets").join("shaders").join("rim.wgsl");
    assert_eq!(refresh(&shipped_wgsl), Refresh::Current);
}

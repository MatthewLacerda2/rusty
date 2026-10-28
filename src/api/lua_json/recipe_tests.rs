//! The texture and shader recipes as their Lua verbs read them (#1209): the
//! on-disk JSON (`to_json`) comes back unchanged through [`recipe_from_lua`], and
//! an unknown key is refused by name.

use std::collections::BTreeMap;

use mlua::{Lua, Value};

use super::*;
use crate::procgen::recipe::{Checker, Node, OpKind};
use crate::procgen::TextureRecipe;
use crate::shadergen::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};

/// Read `json` the way a script passing it as a string to a recipe verb is read.
fn read<T: DeserializeOwned>(lua: &Lua, json: &str) -> Result<T, String> {
    recipe_from_lua(&Value::String(lua.create_string(json).unwrap()))
}

fn texture() -> TextureRecipe {
    let node = |id: &str, op, inputs: &[&str]| Node {
        id: id.into(),
        op,
        inputs: inputs.iter().map(|s| s.to_string()).collect(),
    };
    let checker = OpKind::Checker(Checker {
        tiles: 4,
        color_a: [0.0, 0.0, 0.0, 1.0],
        color_b: [1.0, 1.0, 1.0, 1.0],
    });
    TextureRecipe::new(
        64,
        vec![
            node("n0", checker, &[]),
            node("n1", OpKind::Invert, &["n0"]),
        ],
    )
    .with_seed(7)
    .with_output("n1")
}

fn shader() -> ShaderRecipe {
    let params = BTreeMap::from([("steps".to_string(), ParamValue::Scalar(4.0))]);
    ShaderRecipe {
        pass: PassKind::Surface,
        name: "toon".into(),
        blocks: vec![BlockSel {
            id: "toon_ramp".into(),
            params,
        }],
    }
}

#[test]
fn recipes_round_trip_through_their_json_unchanged() {
    let lua = Lua::new();
    let single = texture();
    let back: TextureRecipe = read(&lua, &single.to_json().unwrap()).unwrap();
    assert_eq!(back, single);
    let mut multi = texture();
    multi.outputs.insert("base_color".into(), "n0".into());
    multi.outputs.insert("normal".into(), "n1".into());
    let back: TextureRecipe = read(&lua, &multi.to_json().unwrap()).unwrap();
    assert_eq!(back, multi);
    let back: ShaderRecipe = read(&lua, &shader().to_json().unwrap()).unwrap();
    assert_eq!(back, shader());
}

#[test]
fn an_unknown_recipe_or_block_key_is_refused_by_name() {
    let lua = Lua::new();
    let texture = r#"{ "resolution": 8, "nodes": [], "seeed": 3 }"#;
    let err = read::<TextureRecipe>(&lua, texture).unwrap_err();
    assert!(err.contains("seeed"), "{err}");
    let block = r#"{ "pass": "postfx", "name": "t", "blocks": [{ "id": "tint", "param": {} }] }"#;
    let err = read::<ShaderRecipe>(&lua, block).unwrap_err();
    assert!(err.contains("param"), "{err}");
    let recipe = r#"{ "pass": "postfx", "name": "t", "block": [] }"#;
    let err = read::<ShaderRecipe>(&lua, recipe).unwrap_err();
    assert!(err.contains("block"), "{err}");
}

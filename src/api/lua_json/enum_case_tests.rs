//! Every enum a Lua table reaches through serde reads its name in any case (#1216):
//! the UI gradient kind, every texture op and op enum (walked from the op catalog),
//! the shader pass, and zimmer's patch enums — each decoded from a Lua table.

use mlua::{Lua, Value};
use serde_json::{json, Map, Value as Json};

use super::*;
use crate::components::{GradientKind, UiGradient};
use crate::procgen::recipe::{OpInfo, OpKind, OPS};
use crate::procgen::TextureRecipe;
use crate::shadergen::{PassKind, ShaderRecipe};

fn eval(lua: &Lua, src: &str) -> Value {
    lua.load(src).eval().unwrap()
}

#[test]
fn a_gradient_kind_reads_in_any_case_from_lua() {
    let lua = Lua::new();
    for kind in ["Radial", "radial", "RADIAL"] {
        let src = format!("return {{ kind = '{kind}' }}");
        let g: UiGradient = recipe_from_lua(&eval(&lua, &src)).unwrap();
        assert_eq!(g.kind, GradientKind::Radial, "{kind}");
    }
    let err = recipe_from_lua::<UiGradient>(&eval(&lua, "return { kind = 'Conic' }"));
    let plain = serde_json::from_value::<UiGradient>(json!({ "kind": "Conic" })).unwrap_err();
    assert_eq!(
        err.unwrap_err(),
        plain.to_string(),
        "unknown stays serde's error"
    );
}

/// A valid value for a required non-enum param of type `ty`.
fn sample(ty: &str) -> Json {
    match ty {
        "color" => json!([0.0, 0.0, 0.0, 1.0]),
        "vec2" => json!([1.0, 1.0]),
        "bool" => json!(true),
        "stops" => json!([{ "pos": 0.0, "color": [0.0, 0.0, 0.0, 1.0] }]),
        _ => json!(2),
    }
}

/// `info`'s op with its required params filled and every enum param at its
/// `round`th value, the op tag and the enum values passed through `case`.
fn op_doc(info: &OpInfo, round: usize, case: fn(&str) -> String) -> Json {
    let mut doc = Map::new();
    doc.insert("op".into(), json!(case(info.op)));
    for p in info.params {
        let value = match p.ty {
            "enum" => json!(case(p.values[round % p.values.len()])),
            _ if p.default.is_none() => sample(p.ty),
            _ => continue,
        };
        doc.insert(p.name.into(), value);
    }
    Json::Object(doc)
}

#[test]
fn every_texture_op_and_op_enum_reads_in_any_case() {
    for info in OPS {
        let enums = info.params.iter().filter(|p| p.ty == "enum");
        let rounds = enums.map(|p| p.values.len()).max().unwrap_or(1);
        for round in 0..rounds {
            let canonical = op_doc(info, round, str::to_owned);
            let expected: OpKind = serde_json::from_value(canonical)
                .unwrap_or_else(|e| panic!("{}: canonical doc fails: {e}", info.op));
            let shouted = decode::<OpKind>(op_doc(info, round, str::to_uppercase));
            assert_eq!(shouted.unwrap(), expected, "{} round {round}", info.op);
        }
    }
}

#[test]
fn a_texture_recipe_from_lua_reads_its_op_in_any_case() {
    let lua = Lua::new();
    let src = "return { resolution = 8, output = 'n0', nodes = {
        { id = 'n0', op = 'NOISE', kind = 'FBM', scale = 4 } } }";
    let r: TextureRecipe = recipe_from_lua(&eval(&lua, src)).unwrap();
    let canon = serde_json::to_value(&r).unwrap();
    assert_eq!(canon["nodes"][0]["op"], "noise");
    assert_eq!(canon["nodes"][0]["kind"], "fbm");
}

#[test]
fn a_shader_pass_reads_in_any_case_from_lua() {
    let lua = Lua::new();
    let src = "return { pass = 'POSTFX', name = 'p' }";
    let r: ShaderRecipe = recipe_from_lua(&eval(&lua, src)).unwrap();
    assert_eq!(r.pass, PassKind::Postfx);
}

#[test]
fn zimmers_patch_enums_read_in_any_case_from_lua() {
    let lua = Lua::new();
    let patch = |source: &str, wave: &str, filter: &str, fx: &str| {
        format!(
            "return {{ source = {{ kind = '{source}', oscs = {{ {{ wave = '{wave}' }} }} }},
              amp = {{ a = 0.005, d = 0.1, s = 0.7, r = 0.3 }}, filter = {{ kind = '{filter}', cutoff = 1200, resonance = 0.4, env_octaves = 2 }}, fx = {{ {{ fx = '{fx}', size = 0.6, damp = 0.5, mix = 0.2 }} }} }}"
        )
    };
    let canonical = patch("osc_stack", "saw", "lowpass", "reverb");
    let shouted = patch("OSC_STACK", "Saw", "LowPass", "REVERB");
    let a: zimmer::Patch = recipe_from_lua(&eval(&lua, &canonical)).unwrap();
    let b: zimmer::Patch = recipe_from_lua(&eval(&lua, &shouted)).unwrap();
    assert_eq!(
        serde_json::to_value(a).unwrap(),
        serde_json::to_value(b).unwrap()
    );
}

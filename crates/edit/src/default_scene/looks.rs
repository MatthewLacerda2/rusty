use std::collections::BTreeMap;

use crate::authoring::material::{self as mat, MaterialLibrary};
use rusty_components::RenderFace;
use rusty_procgen::recipe::{Checker, Node, OpKind};
use rusty_procgen::TextureRecipe;
use rusty_scene::{MaterialAsset, RenderMode};
use rusty_shadergen::recipe::ParamValue;
use rusty_shadergen::{BlockSel, PassKind, ShaderRecipe};

pub const FLOOR: &str = "checker";
pub const DECK: &str = "deck";
pub const PLAYER: &str = "plastic_blue";
pub const WALL: &str = "concrete";
pub const ENEMY: &str = "enemy_red";
pub const CRATE: &str = "crate";
pub const WATER: &str = "water";

/// Where `bake_maps` writes `FLOOR`'s map (`<MAPS_DIR>/<FLOOR>_base_color.png`): change them
/// together.
pub const CHECKER_MAP: &str = "assets/textures/checker_base_color.png";
pub const SHADER_NAME: &str = "default_rim";

pub fn checker_recipe() -> TextureRecipe {
    // Two neutral greys, so each material's base colour tints the checker.
    let checker = Node {
        id: "checker".into(),
        op: OpKind::Checker(Checker {
            tiles: 16,
            color_a: [0.82, 0.82, 0.82, 1.0],
            color_b: [0.48, 0.48, 0.48, 1.0],
        }),
        inputs: vec![],
    };
    let mut recipe = TextureRecipe::new(512, vec![checker]);
    recipe.outputs = BTreeMap::from([("base_color".into(), "checker".into())]);
    recipe
}

pub fn shader_recipe() -> ShaderRecipe {
    let params = BTreeMap::from([
        ("color".into(), ParamValue::Vector(vec![1.0, 0.55, 0.25])),
        ("power".into(), ParamValue::Scalar(2.5)),
        ("strength".into(), ParamValue::Scalar(0.9)),
    ]);
    ShaderRecipe {
        pass: PassKind::Surface,
        name: SHADER_NAME.into(),
        blocks: vec![BlockSel {
            id: "fresnel_rim".into(),
            params,
        }],
    }
}

struct Look {
    key: &'static str,
    color: [f32; 3],
    metallic: f32,
    roughness: f32,
    checker: bool,
}

const LOOKS: &[Look] = &[
    look(FLOOR, [0.8, 0.9, 0.98], 0.0, 0.3, true),
    look(DECK, [0.86, 0.8, 0.68], 0.0, 0.9, true),
    look(PLAYER, [0.3, 0.6, 1.0], 0.0, 0.35, false),
    look(WALL, [0.78, 0.74, 0.68], 0.0, 0.9, true),
    look(ENEMY, [0.85, 0.12, 0.12], 0.1, 0.45, false),
    look(CRATE, [0.72, 0.5, 0.3], 0.0, 0.7, true),
    look(WATER, [0.2, 0.55, 0.85], 0.0, 0.05, false),
];

const fn look(
    key: &'static str,
    color: [f32; 3],
    metallic: f32,
    roughness: f32,
    checker: bool,
) -> Look {
    Look {
        key,
        color,
        metallic,
        roughness,
        checker,
    }
}

pub(super) fn define_all(materials: &mut MaterialLibrary) {
    let floor = MaterialAsset {
        maps_recipe: Some(checker_recipe()),
        ..MaterialAsset::default()
    };
    mat::define_asset(materials, FLOOR, floor);
    for l in LOOKS {
        mat::ensure_named(materials, l.key);
        mat::set_base_color(materials, l.key, l.color);
        mat::set_metallic(materials, l.key, l.metallic);
        mat::set_roughness(materials, l.key, l.roughness);
        // Set by hand, so building does no I/O; a bake fills only empty map fields and keeps it.
        if l.checker {
            mat::set_base_color_map(materials, l.key, CHECKER_MAP.to_string());
        }
    }
    mat::set_shader(materials, ENEMY, SHADER_NAME.to_string());
    mat::set_render_mode(materials, WATER, RenderMode::Transparent);
    mat::set_alpha(materials, WATER, 0.45);
    // A swimmer looks up at the surface too.
    mat::set_render_face(materials, WATER, RenderFace::Both);
}

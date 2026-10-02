//! src/scene/default_scene/looks.rs — the default scene's materials, its checker
//! texture recipe and its baked surface shader recipe (#667).
//!
//! The Unity workflow, kept small: one texture and one authored shader, and a few
//! materials made from them with different tints, metallic and roughness. Every
//! material is written through `authoring::material`, the ops the inspector and
//! `Material.*` share.

use std::collections::BTreeMap;

use crate::procgen::recipe::{Node, OpKind};
use crate::procgen::TextureRecipe;
use crate::scene::authoring::material::{self as mat, MaterialLibrary};
use crate::scene::MaterialAsset;
use crate::shadergen::recipe::ParamValue;
use crate::shadergen::{BlockSel, PassKind, ShaderRecipe};

/// The checkerboard floor material; it carries the checker recipe (its `maps`), so
/// `Material.Rebake("checker")` regenerates the texture.
pub const FLOOR: &str = "checker";
pub const PLAYER: &str = "plastic_blue";
pub const WALL: &str = "concrete";
pub const ENEMY: &str = "enemy_red";
pub const CRATE: &str = "crate";
pub const METAL: &str = "metal";

/// Where the checker bakes: `<MAPS_DIR>/<FLOOR>_base_color.png`, the path
/// `authoring::material::bake_maps` writes for the floor material.
pub const CHECKER_MAP: &str = "project/assets/textures/checker_base_color.png";
/// The default surface shader's module name: `project/assets/shaders/<name>.wgsl`.
pub const SHADER_NAME: &str = "default_rim";

/// The checkerboard: two soft greys, 16 tiles across a 512 px tile. Neutral, so a
/// material's base colour tints it. Built on the procgen `Checker` op — the same
/// generator `Texture.Bake` uses — so the engine keeps one checker.
pub fn checker_recipe() -> TextureRecipe {
    let checker = Node {
        id: "checker".into(),
        op: OpKind::Checker {
            tiles: 16,
            color_a: [0.82, 0.82, 0.82, 1.0],
            color_b: [0.48, 0.48, 0.48, 1.0],
        },
        inputs: vec![],
    };
    let mut recipe = TextureRecipe::new(512, vec![checker]);
    recipe.outputs = BTreeMap::from([("base_color".into(), "checker".into())]);
    recipe
}

/// The default surface shader: the standard PBR look plus a warm Fresnel rim, so the
/// enemy reads against any background. `color` and `strength` are runtime params
/// (`Material.SetShaderParam`); `power` is baked.
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

/// One material's look: tint, metallic, roughness, and whether it wears the checker.
struct Look {
    key: &'static str,
    color: [f32; 3],
    metallic: f32,
    roughness: f32,
    checker: bool,
}

const LOOKS: &[Look] = &[
    look(FLOOR, [0.86, 0.88, 0.92], 0.0, 0.85, true),
    look(PLAYER, [0.3, 0.6, 1.0], 0.0, 0.35, false),
    look(WALL, [0.78, 0.74, 0.68], 0.0, 0.9, true),
    look(ENEMY, [0.85, 0.12, 0.12], 0.1, 0.45, false),
    look(CRATE, [0.72, 0.5, 0.3], 0.0, 0.7, true),
    look(METAL, [0.9, 0.9, 0.92], 0.55, 0.3, false),
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

/// Define every default material in `materials`. The floor's carries the checker
/// recipe; the map path is set by hand, so building stays free of I/O and a bake
/// (which fills only empty map fields) writes exactly there.
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
        if l.checker {
            mat::set_base_color_map(materials, l.key, CHECKER_MAP.to_string());
        }
    }
    mat::set_shader(materials, ENEMY, SHADER_NAME.to_string());
}

mod component;
mod decal;
mod defaults;
mod detail;
mod height;
mod mapping;
mod surface;
mod temporal;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use component::{entity_material_key, MaterialComponent};
pub use decal::DecalBlend;
use defaults::{
    default_alpha, default_alpha_cutoff, default_base_color, default_roughness, default_true,
};
pub use detail::DetailLayer;
use height::default_height_scale;
pub use height::HEIGHT_SCALE_MAX;
pub use mapping::UvMapping;
use mapping::{default_blend_sharpness, default_tiling};
use rusty_asset::MapRef;
use rusty_procgen::TextureRecipe;
use surface::{default_ior, default_one, white};
pub use surface::{BlendMode, RefractionModel, RenderFace, RenderMode};
pub use temporal::{TaaReactive, MAX_REACTIVE};

pub const SORTING_PRIORITY_RANGE: i32 = 50;

fn is_true(value: &bool) -> bool {
    *value
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaterialAsset {
    #[serde(default = "default_base_color")]
    pub base_color: [f32; 3],
    #[serde(default)]
    pub base_color_map: Option<MapRef>,
    #[serde(default)]
    pub metallic: f32,
    #[serde(default)]
    pub metallic_map: Option<MapRef>,
    #[serde(default = "default_roughness")]
    pub roughness: f32,
    #[serde(default)]
    pub roughness_map: Option<MapRef>,
    #[serde(default)]
    pub normal_map: Option<MapRef>,
    #[serde(default)]
    pub emissive: [f32; 3],
    #[serde(default)]
    pub emissive_map: Option<MapRef>,
    #[serde(default)]
    pub occlusion_map: Option<MapRef>,
    #[serde(default = "default_one")]
    pub occlusion_strength: f32,
    #[serde(default)]
    pub specular_map: Option<MapRef>,
    #[serde(default)]
    pub height_map: Option<MapRef>,
    #[serde(default = "default_height_scale")]
    pub height_scale: f32,
    #[serde(default = "default_tiling")]
    pub tiling: [f32; 2],
    #[serde(default)]
    pub offset: [f32; 2],
    #[serde(default)]
    pub uv_mapping: UvMapping,
    #[serde(default = "default_one")]
    pub world_size: f32,
    #[serde(default = "default_blend_sharpness")]
    pub blend_sharpness: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<DetailLayer>,
    #[serde(default)]
    pub render_mode: RenderMode,
    #[serde(default)]
    pub render_face: RenderFace,
    #[serde(default = "default_alpha")]
    pub alpha: f32,
    #[serde(default = "default_alpha_cutoff")]
    pub alpha_cutoff: f32,
    #[serde(default)]
    pub blend_mode: BlendMode,
    #[serde(default = "default_true")]
    pub preserve_specular: bool,
    #[serde(default = "default_true")]
    pub receive_shadows: bool,
    #[serde(default)]
    pub refraction_model: RefractionModel,
    #[serde(default)]
    pub sorting_priority: i32,
    #[serde(default = "default_ior")]
    pub ior: f32,
    #[serde(default = "default_one")]
    pub specular: f32,
    #[serde(default = "white")]
    pub specular_color: [f32; 3],
    #[serde(default = "default_one")]
    pub thickness: f32,
    #[serde(default = "white")]
    pub transmittance_color: [f32; 3],
    #[serde(default = "default_one")]
    pub absorption_distance: f32,
    #[serde(default)]
    pub depth_postpass: bool,
    #[serde(default)]
    pub writes_motion: bool,
    #[serde(default)]
    pub reactive: TaaReactive,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receive_ssr: Option<bool>,
    #[serde(default = "white")]
    pub transmission_color: [f32; 3],
    #[serde(default)]
    pub transmission: f32,
    #[serde(default)]
    pub shader: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shader_params: BTreeMap<String, Vec<f32>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shader_textures: BTreeMap<String, MapRef>,
    #[serde(default, rename = "maps", skip_serializing_if = "Option::is_none")]
    pub maps_recipe: Option<TextureRecipe>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub receive_decals: bool,
    #[serde(default, skip_serializing_if = "DecalBlend::is_default")]
    pub decal: DecalBlend,
}

impl MaterialAsset {
    pub fn is_transparent(&self) -> bool {
        self.render_mode == RenderMode::Transparent
    }

    pub fn sort_priority(&self) -> i32 {
        let range = SORTING_PRIORITY_RANGE;
        self.sorting_priority.clamp(-range, range)
    }

    pub fn receives_ssr(&self) -> bool {
        self.receive_ssr.unwrap_or(!self.is_transparent())
    }

    pub fn occlusion_amount(&self) -> f32 {
        match self.occlusion_strength.is_nan() {
            true => 1.0,
            false => self.occlusion_strength.clamp(0.0, 1.0),
        }
    }
}

#[cfg(test)]
mod tests;

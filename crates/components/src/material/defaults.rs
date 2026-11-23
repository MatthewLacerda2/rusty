use std::collections::BTreeMap;

use super::{
    default_blend_sharpness, default_height_max_steps, default_height_min_steps,
    default_height_scale, default_ior, default_one, default_tiling, white, BlendMode, DecalBlend,
    MaterialAsset, RefractionModel, RenderFace, RenderMode, TaaReactive, UvMapping,
};

pub(super) fn default_base_color() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

pub(super) fn default_roughness() -> f32 {
    0.5
}

pub(super) fn default_alpha() -> f32 {
    1.0
}

pub(super) fn default_alpha_cutoff() -> f32 {
    0.5
}

pub(super) fn default_true() -> bool {
    true
}

impl Default for MaterialAsset {
    fn default() -> Self {
        Self {
            base_color: default_base_color(),
            base_color_map: None,
            metallic: 0.0,
            metallic_map: None,
            roughness: default_roughness(),
            roughness_map: None,
            normal_map: None,
            emissive: [0.0, 0.0, 0.0],
            emissive_map: None,
            occlusion_map: None,
            occlusion_strength: default_one(),
            specular_map: None,
            height_map: None,
            height_scale: default_height_scale(),
            height_min_steps: default_height_min_steps(),
            height_max_steps: default_height_max_steps(),
            tiling: default_tiling(),
            offset: [0.0; 2],
            uv_mapping: UvMapping::Uv0,
            world_size: default_one(),
            blend_sharpness: default_blend_sharpness(),
            detail: None,
            render_mode: RenderMode::Opaque,
            render_face: RenderFace::Front,
            alpha: default_alpha(),
            alpha_cutoff: default_alpha_cutoff(),
            blend_mode: BlendMode::Alpha,
            preserve_specular: true,
            receive_shadows: true,
            refraction_model: RefractionModel::None,
            sorting_priority: 0,
            ior: default_ior(),
            specular: default_one(),
            specular_color: white(),
            thickness: default_one(),
            transmittance_color: white(),
            absorption_distance: default_one(),
            depth_postpass: false,
            writes_motion: false,
            reactive: TaaReactive::Auto,
            receive_ssr: None,
            transmission_color: white(),
            transmission: 0.0,
            shader: None,
            shader_params: BTreeMap::new(),
            shader_textures: BTreeMap::new(),
            maps_recipe: None,
            receive_decals: true,
            decal: DecalBlend::default(),
        }
    }
}

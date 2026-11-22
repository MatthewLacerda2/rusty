//! Adding a material field: spend a `_pad` word or append a 16-byte run to [`MaterialData`],
//! mirror it in `forward/bindings.wgsl`, set it in `render::draw::uniforms::material_data`
//! (and `RecordTraits` if a pass is decided by it, or `records::record_free` if read alone),
//! and add its row to `uniform_layout_tests::draw`.

/// Mirrors `DrawUniforms` in `engine/shaders/forward/bindings.wgsl`: change both together.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DrawUniform {
    pub model_matrix: [f32; 16],
    pub bone_base: u32,
    pub material: u32,
    pub _pad: [u32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MaterialData {
    pub color_tint: [f32; 4],
    pub use_texture: u32,
    pub is_lit: u32,
    pub metallic: f32,
    pub roughness: f32,
    pub use_metallic_map: u32,
    pub use_roughness_map: u32,
    /// 0 no map, 1 an 8-bit map keeping z, 2 BC5 (spread in `normal_variance`), 3 16-bit signed.
    pub use_normal_map: u32,
    pub use_emissive_map: u32,
    pub emissive: [f32; 4],
    pub use_cutout: u32,
    pub alpha_cutoff: f32,
    pub receive_decals: u32,
    pub two_sided: u32,
    pub surface: SurfaceUniform,
    pub mapping: MappingUniform,
    pub normal_variance: [u32; 4],
    pub dielectric_reflectance: [f32; 4],
    pub detail: DetailUniform,
    pub level_maps: LevelMapsUniform,
}

// Bytes: the per-draw stride, wgpu's 256-byte dynamic-offset alignment. The record has no cap.
const _: () = assert!(std::mem::size_of::<DrawUniform>() <= 256);
const _: () = assert!(std::mem::size_of::<MaterialData>().is_multiple_of(16));

pub(crate) const MATERIAL_DATA_WORDS: usize = std::mem::size_of::<MaterialData>() / 4;

impl MaterialData {
    pub(crate) fn words(&self) -> [u32; MATERIAL_DATA_WORDS] {
        let mut words = [0u32; MATERIAL_DATA_WORDS];
        words.copy_from_slice(bytemuck::cast_slice(bytemuck::bytes_of(self)));
        words
    }

    pub(crate) fn from_words(words: &[u32; MATERIAL_DATA_WORDS]) -> Self {
        bytemuck::pod_read_unaligned(bytemuck::cast_slice(words))
    }

    pub(crate) fn flat(tint: [f32; 4]) -> Self {
        Self {
            color_tint: tint,
            roughness: 0.5,
            surface: SurfaceUniform::default(),
            mapping: MappingUniform::default(),
            ..bytemuck::Zeroable::zeroed()
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SurfaceUniform {
    pub blend_mode: u32,
    pub premultiply_diffuse: u32,
    pub receive_shadows: u32,
    pub refraction_model: u32,
    pub ior: f32,
    pub thickness: f32,
    pub uses_opaque: u32,
    pub depth_postpass: u32,
    pub absorption: [f32; 4],
    pub transmission: [f32; 4],
    pub receive_ssr: u32,
    pub reactive: f32,
    pub writes_motion: u32,
    pub _pad: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MappingUniform {
    pub uv_st: [f32; 4],
    pub uv_mapping: u32,
    pub world_scale: f32,
    pub blend_sharpness: f32,
    pub _pad: u32,
}

impl Default for MappingUniform {
    fn default() -> Self {
        Self {
            uv_st: [1.0, 1.0, 0.0, 0.0],
            uv_mapping: 0,
            world_scale: 1.0,
            blend_sharpness: 4.0,
            _pad: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DetailUniform {
    pub detail_st: [f32; 4],
    pub use_detail_map: u32,
    pub detail_world_scale: f32,
    pub detail_albedo_scale: f32,
    pub detail_normal_scale: f32,
    pub detail_smoothness_scale: f32,
    pub use_detail_mask: u32,
    pub _pad: [u32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LevelMapsUniform {
    pub use_occlusion_map: u32,
    pub occlusion_strength: f32,
    pub use_specular_map: u32,
    pub use_height_map: u32,
    pub height_scale: f32,
    pub _pad: [u32; 3],
}

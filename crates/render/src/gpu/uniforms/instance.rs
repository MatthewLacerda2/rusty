#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ObjectData {
    pub model_matrix: [f32; 16],
    pub use_sh: u32,
    pub lightmap_page: u32,
    pub lightmap_directional: u32,
    pub in_bake: u32,
    pub lightmap_st: [f32; 4],
    pub sh: [[f32; 4]; 9],
    pub occlusion: [f32; 4],
}

impl ObjectData {
    pub(crate) const IDENTITY: Self = Self {
        model_matrix: glam::Mat4::IDENTITY.to_cols_array(),
        use_sh: 0,
        lightmap_page: 0,
        lightmap_directional: 0,
        in_bake: 0,
        lightmap_st: [0.0; 4],
        sh: [[0.0; 4]; 9],
        occlusion: [1.0; 4],
    };
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct InstanceData {
    pub object: u32,
    pub reflection_probe: u32,
    pub proxy_volume: u32,
    pub motion: u32,
}

impl InstanceData {
    pub(crate) const fn of(object: u32) -> Self {
        Self {
            object,
            reflection_probe: 0,
            proxy_volume: 0,
            motion: 0,
        }
    }

    pub(crate) const IDENTITY: Self = Self::of(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_keep_the_shader_layouts_size() {
        assert_eq!(
            std::mem::size_of::<ObjectData>(),
            64 + 16 + 16 + 9 * 16 + 16
        );
        assert_eq!(std::mem::size_of::<InstanceData>(), 16);
    }
}

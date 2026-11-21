rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum UvMapping {
        #[default]
        Uv0 = "UV0",
        Planar = "Planar",
        Triplanar = "Triplanar",
    }
}

pub(super) fn default_tiling() -> [f32; 2] {
    [1.0, 1.0]
}

pub(super) fn default_blend_sharpness() -> f32 {
    4.0
}

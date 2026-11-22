rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum RenderMode {
        #[default]
        Opaque = "Opaque",
        Cutout = "Cutout",
        Transparent = "Transparent",
    }
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum RenderFace {
        #[default]
        Front = "Front",
        Back = "Back",
        Both = "Both",
    }
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum BlendMode {
        #[default]
        Alpha = "Alpha",
        Premultiply = "Premultiply",
        Additive = "Additive",
        Multiply = "Multiply",
    }
}

impl BlendMode {
    pub fn preserves_specular(self) -> bool {
        matches!(self, Self::Alpha | Self::Additive)
    }
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum RefractionModel {
        #[default]
        None = "None",
        Planar = "Planar",
        Thin = "Thin",
    }
}

pub(super) fn default_ior() -> f32 {
    1.5
}

pub(super) fn default_one() -> f32 {
    1.0
}

pub(super) fn white() -> [f32; 3] {
    [1.0; 3]
}

pub fn dielectric_f0(ior: f32, specular: f32, color: [f32; 3]) -> [f32; 3] {
    let n = within(ior, 1.0, f64::MAX);
    let reflectance = ((n - 1.0) / (n + 1.0)).powi(2);
    let strength = within(specular, 0.0, 1.0);
    color.map(|c| ((reflectance * within(c, 0.0, f64::MAX)).min(1.0) * strength) as f32)
}

pub fn dielectric_f90(specular: f32) -> f32 {
    within(specular, 0.0, 1.0) as f32
}

fn within(v: f32, lo: f64, hi: f64) -> f64 {
    if v.is_nan() {
        lo
    } else {
        f64::from(v).clamp(lo, hi)
    }
}

impl super::MaterialAsset {
    pub fn dielectric_f0(&self) -> [f32; 3] {
        dielectric_f0(self.ior, self.specular, self.specular_color)
    }

    pub fn dielectric_f90(&self) -> f32 {
        dielectric_f90(self.specular)
    }

    pub fn reads_opaque(&self) -> bool {
        let blends_over = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::Premultiply);
        let refracts = self.refraction_model != RefractionModel::None;
        let absorbs = self.transmittance_color != white();
        self.is_transparent() && blends_over && (refracts || absorbs)
    }
}

#[cfg(test)]
#[path = "f0_tests.rs"]
mod f0_tests;

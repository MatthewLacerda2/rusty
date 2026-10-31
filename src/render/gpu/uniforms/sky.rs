//! The procedural sky's settings as the shaders read them (#1620): `ProceduralSky` in
//! `common.wgsl`, carried by the lighting uniform (the sky pass) and by the sky
//! cube's bake params (`sky/prefilter.wgsl`), so the two draw one sky.

use crate::scene::SkySettings;

/// Mirrors WGSL `ProceduralSky` byte-for-byte. Each colour's `w` is 1 when the scene
/// sets it, 0 when it follows the ambient colour (the shader derives it then).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ProceduralSkyUniform {
    pub zenith: [f32; 4],
    pub horizon: [f32; 4],
    pub ground: [f32; 4],
    /// x: cloud coverage, y: cloud opacity.
    pub clouds: [f32; 4],
}

impl ProceduralSkyUniform {
    pub(crate) fn from_settings(sky: &SkySettings) -> Self {
        let color = |c: Option<glam::Vec3>| match c {
            Some(c) => c.extend(1.0).to_array(),
            None => [0.0; 4],
        };
        Self {
            zenith: color(sky.zenith),
            horizon: color(sky.horizon),
            ground: color(sky.ground),
            clouds: [sky.cloud_coverage, sky.cloud_opacity, 0.0, 0.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unset_colour_follows_the_ambient_and_a_set_one_is_flagged() {
        let mut sky = SkySettings::default();
        let default = ProceduralSkyUniform::from_settings(&sky);
        assert_eq!(default.zenith, [0.0; 4]);
        assert_eq!(default.clouds, [0.5, 0.6, 0.0, 0.0]);
        sky.ground = Some(glam::Vec3::new(0.1, 0.2, 0.3));
        let set = ProceduralSkyUniform::from_settings(&sky);
        assert_eq!(set.ground, [0.1, 0.2, 0.3, 1.0]);
        assert_ne!(set, default, "a change is seen by the cube's bake plan");
    }
}

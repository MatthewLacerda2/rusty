//! One resident probe as the shader reads it (#1206), and the luminance SH of its
//! stored irradiance the forward pass normalises its reflection by (#1697).

use crate::core::color::luminance;
use crate::scene::lighting::sh::Sh9;
use crate::scene::ReflectionProbe;

/// One resident probe as the shader reads it: its capture point, its parallax box,
/// its cube's array layer, where the cube's mip chain sits in it, its intensity,
/// box-projection, fallback and normalisation switches, and the luminance SH of its
/// stored irradiance. Mirrors `ReflectionProbe` in
/// `engine/shaders/lighting/reflection_probe.wgsl` byte-for-byte.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ReflectionProbeUniform {
    pub center: [f32; 3],
    /// The array mip holding this cube's base level.
    pub mip_base: f32,
    pub box_min: [f32; 3],
    /// How many mips its chain has.
    pub mip_count: f32,
    pub box_max: [f32; 3],
    /// The array layer (cube) its cube was copied into.
    pub layer: u32,
    /// `x` intensity, `y` 1 when box projection is on, `z` 1 for the scene's
    /// fallback probe (only ever slot 0, #1697), `w` 1 when `irradiance` holds a
    /// stored irradiance the reflection is normalised by.
    pub params: [f32; 4],
    /// The 9 L2 SH coefficients of the stored irradiance's luminance, convolved and
    /// over π ([`Sh9::convolved_over_pi`]), so one dot product with the basis along
    /// `N` gives E/π, the unit the lightmap and the light-probe term hold.
    pub irradiance: [[f32; 4]; 3],
}

impl ReflectionProbeUniform {
    /// `probe` as resident in `layer`, its cube's chain of `mips` starting `shift`
    /// mips below the array's base.
    pub(super) fn of(probe: &ReflectionProbe, shift: u32, mips: u32, layer: u32) -> Self {
        let flag = |on: bool| f32::from(u8::from(on));
        Self {
            center: probe.position.to_array(),
            mip_base: shift as f32,
            box_min: probe.box_min.to_array(),
            mip_count: mips as f32,
            box_max: probe.box_max.to_array(),
            layer,
            params: [
                probe.intensity,
                flag(probe.box_projection),
                flag(probe.fallback),
                flag(probe.irradiance.is_some()),
            ],
            irradiance: probe.irradiance.map_or([[0.0; 4]; 3], luminance_sh),
        }
    }
}

/// `sh`'s luminance per coefficient, convolved over π, packed four to a `vec4`. SH
/// is linear, so its dot product with the basis is the luminance of the RGB
/// irradiance it would give (unclamped).
pub(crate) fn luminance_sh(sh: Sh9) -> [[f32; 4]; 3] {
    let lum = sh.convolved_over_pi().coeffs.map(luminance);
    [
        [lum[0], lum[1], lum[2], lum[3]],
        [lum[4], lum[5], lum[6], lum[7]],
        [lum[8], 0.0, 0.0, 0.0],
    ]
}

#[cfg(test)]
#[path = "uniform_tests.rs"]
mod uniform_tests;

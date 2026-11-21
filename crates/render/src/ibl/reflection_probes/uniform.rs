use rusty_core::color::luminance;
use rusty_scene::lighting::sh::Sh9;
use rusty_scene::ReflectionProbe;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ReflectionProbeUniform {
    pub center: [f32; 3],
    pub mip_base: f32,
    pub box_min: [f32; 3],
    pub mip_count: f32,
    pub box_max: [f32; 3],
    pub layer: u32,
    pub params: [f32; 4],
    pub irradiance: [[f32; 4]; 3],
    pub rotation: [f32; 4],
    pub shape: [f32; 4],
}

impl ReflectionProbeUniform {
    pub(super) fn of(probe: &ReflectionProbe, shift: u32, mips: u32, layer: u32) -> Self {
        let flag = |on: bool| f32::from(u8::from(on));
        let frame = probe.frame();
        Self {
            center: frame.center.to_array(),
            mip_base: shift as f32,
            box_min: frame.box_min.to_array(),
            mip_count: mips as f32,
            box_max: frame.box_max.to_array(),
            layer,
            params: [
                probe.intensity,
                flag(probe.box_projection && !probe.fallback),
                flag(probe.fallback),
                flag(probe.irradiance.is_some()),
            ],
            irradiance: probe.irradiance.map_or([[0.0; 4]; 3], luminance_sh),
            rotation: frame.inverse.to_array(),
            shape: [
                flag(frame.sphere.is_some()),
                frame.sphere.unwrap_or(0.0),
                frame.influence,
                frame.fade,
            ],
        }
    }
}

pub(crate) fn luminance_sh(sh: Sh9) -> [[f32; 4]; 3] {
    let lum = sh.convolved_over_pi().coeffs.map(luminance);
    [
        [lum[0], lum[1], lum[2], lum[3]],
        [lum[4], lum[5], lum[6], lum[7]],
        [lum[8], 0.0, 0.0, 0.0],
    ]
}

#[cfg(test)]
pub(crate) fn normalisation(packed: [[f32; 4]; 3], n: glam::Vec3, baked: f32) -> f32 {
    let flat = packed.as_flattened();
    let sh = Sh9 {
        coeffs: std::array::from_fn(|i| [flat[i]; 3]),
    };
    let stored = sh.eval_radiance(n).x;
    let stored = stored.max(flat[0] * SH_C0 * STORED_FLOOR);
    ((baked + NORMALISE_BIAS) / (stored + NORMALISE_BIAS)).clamp(0.0, 1.0)
}

#[cfg(test)]
pub(crate) const STORED_FLOOR: f32 = 0.25;
#[cfg(test)]
pub(crate) const NORMALISE_BIAS: f32 = 0.001;
#[cfg(test)]
const SH_C0: f32 = 0.282_094_8;

#[cfg(test)]
#[path = "uniform_tests.rs"]
mod uniform_tests;

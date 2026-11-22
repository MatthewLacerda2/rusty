use super::{Effect, Knob};
use crate::authoring::fields::table::effect;
use rusty_components::{PostProcessingComponent, VolumetricFog};
use rusty_core::settings::quality::EffectQuality;
use rusty_scene::rules::sanitize::{clamped, finite_array_or, unit};

type Fog = VolumetricFog;

pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.volumetric_fog.active = active;
}

pub fn set_mean_free_path(vc: &mut PostProcessingComponent, metres: f32) {
    let v = clamped(metres, Fog::MIN_MEAN_FREE_PATH, Fog::MAX_MEAN_FREE_PATH);
    vc.volumetric_fog.mean_free_path = v;
}

pub fn set_albedo(vc: &mut PostProcessingComponent, albedo: [f32; 3]) {
    let fog = &mut vc.volumetric_fog;
    fog.albedo = finite_array_or(albedo, fog.albedo).map(unit);
}

pub fn set_anisotropy(vc: &mut PostProcessingComponent, g: f32) {
    vc.volumetric_fog.anisotropy = clamped(g, -Fog::MAX_ANISOTROPY, Fog::MAX_ANISOTROPY);
}

pub fn set_base_height(vc: &mut PostProcessingComponent, height: f32) {
    vc.volumetric_fog.base_height = clamped(height, -Fog::MAX_HEIGHT, Fog::MAX_HEIGHT);
}

pub fn set_maximum_height(vc: &mut PostProcessingComponent, height: f32) {
    vc.volumetric_fog.maximum_height = clamped(height, -Fog::MAX_HEIGHT, Fog::MAX_HEIGHT);
}

pub fn set_depth_extent(vc: &mut PostProcessingComponent, metres: f32) {
    let v = clamped(metres, Fog::MIN_DEPTH_EXTENT, Fog::MAX_DEPTH_EXTENT);
    vc.volumetric_fog.depth_extent = v;
}

pub fn set_global_light_probe_dimmer(vc: &mut PostProcessingComponent, dimmer: f32) {
    vc.volumetric_fog.global_light_probe_dimmer = unit(dimmer);
}

pub fn set_quality(vc: &mut PostProcessingComponent, quality: EffectQuality) {
    vc.volumetric_fog.quality = quality;
}

pub fn set_resolution(vc: &mut PostProcessingComponent, froxels: u32) {
    let v = froxels.clamp(Fog::MIN_RESOLUTION, Fog::MAX_RESOLUTION);
    vc.volumetric_fog.resolution = v;
}

pub fn set_slices(vc: &mut PostProcessingComponent, slices: u32) {
    vc.volumetric_fog.slices = slices.clamp(Fog::MIN_SLICES, Fog::MAX_SLICES);
}

pub const EFFECT: Effect = Effect {
    name: "volumetric_fog",
    field: effect!(volumetric_fog, |c, v: VolumetricFog| {
        set_active(c, v.active);
        set_mean_free_path(c, v.mean_free_path);
        set_albedo(c, v.albedo);
        set_anisotropy(c, v.anisotropy);
        set_base_height(c, v.base_height);
        set_maximum_height(c, v.maximum_height);
        set_depth_extent(c, v.depth_extent);
        set_global_light_probe_dimmer(c, v.global_light_probe_dimmer);
        set_quality(c, v.quality);
        set_resolution(c, v.resolution);
        set_slices(c, v.slices);
    }),
    knobs: &[
        Knob {
            field: "mean_free_path",
            api: Some("VolumetricFogMeanFreePath"),
            get: |vc| vc.volumetric_fog.mean_free_path,
            set: set_mean_free_path,
        },
        Knob {
            field: "anisotropy",
            api: Some("VolumetricFogAnisotropy"),
            get: |vc| vc.volumetric_fog.anisotropy,
            set: set_anisotropy,
        },
        Knob {
            field: "base_height",
            api: Some("VolumetricFogBaseHeight"),
            get: |vc| vc.volumetric_fog.base_height,
            set: set_base_height,
        },
        Knob {
            field: "maximum_height",
            api: Some("VolumetricFogMaximumHeight"),
            get: |vc| vc.volumetric_fog.maximum_height,
            set: set_maximum_height,
        },
        Knob {
            field: "depth_extent",
            api: Some("VolumetricFogDepthExtent"),
            get: |vc| vc.volumetric_fog.depth_extent,
            set: set_depth_extent,
        },
        Knob {
            field: "global_light_probe_dimmer",
            api: Some("VolumetricFogGlobalLightProbeDimmer"),
            get: |vc| vc.volumetric_fog.global_light_probe_dimmer,
            set: set_global_light_probe_dimmer,
        },
    ],
};

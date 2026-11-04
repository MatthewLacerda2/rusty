//! Volumetric fog (#1703), HDRP's *Fog* volume override with *Volumetric Fog* on
//! (Frostbite's froxel scattering, as UE, HDRP and Godot 4 draw it): light in the
//! air. Lamps get halos, shadowed lights cast shafts, and a hall fills with haze,
//! lit by the realtime lights with their shadows, the `Baked` lamps unshadowed and
//! the ambient. Off by default; the scene's distance and height fog
//! (`scene::settings::fog`) still draws under it. The quality sets the froxel
//! grid (`render::postfx::effects::volumetric_fog`).
//!
//! Names and defaults are HDRP's: the *Fog Attenuation Distance* (the mean free
//! path), the *Albedo*, the *Anisotropy*, the *Base Height* and *Maximum Height*,
//! the *Volumetric Fog Distance* (`depth_extent`) and the *GI Dimmer*
//! (`global_light_probe_dimmer`).

use serde::{Deserialize, Serialize};

use crate::core::settings::quality::EffectQuality;

/// Whether volumetric fog runs, its medium, and its cost.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VolumetricFog {
    pub active: bool,
    /// HDRP's *Fog Attenuation Distance*: how far light travels through the fog at
    /// its base height before all but `1/e` of it is gone, `1..=10000` metres.
    /// Shorter is thicker.
    pub mean_free_path: f32,
    /// The colour the fog scatters light with (each channel `0..=1`): white keeps
    /// every lamp's own colour, black absorbs without glowing.
    pub albedo: [f32; 3],
    /// Where light scatters, `-1..=1`: `0` evenly, towards `1` mostly onward (a halo
    /// bright looking into a lamp), towards `-1` mostly back (Henyey-Greenstein's g).
    pub anisotropy: f32,
    /// The height below which the fog is at its full density, in world metres.
    pub base_height: f32,
    /// The height above which its density has fallen to `1/e` of the base's,
    /// falling exponentially in between (HDRP's height fog). At least a centimetre
    /// above `base_height` when drawn.
    pub maximum_height: f32,
    /// HDRP's *Volumetric Fog Distance*: how far from the camera the froxels reach,
    /// `4..=1000` metres. A surface past it is fogged as at it.
    pub depth_extent: f32,
    /// How much of the scene's ambient the fog scatters, `0..=1` (HDRP's *GI
    /// Dimmer*): `0` lights it with the lamps and the sun alone.
    pub global_light_probe_dimmer: f32,
    /// Which froxel grid runs: a level of the active quality preset, or `resolution`
    /// and `slices` here when `Custom`.
    pub quality: EffectQuality,
    /// `Custom`: froxels across the view, `16..=256`; up is `9/16` of it.
    pub resolution: u32,
    /// `Custom`: depth slices from the camera to `depth_extent`, `16..=128`.
    pub slices: u32,
}

impl VolumetricFog {
    pub const MIN_MEAN_FREE_PATH: f32 = 1.0;
    pub const MAX_MEAN_FREE_PATH: f32 = 10000.0;
    /// The widest anisotropy a write accepts: Henyey-Greenstein peaks to a point at 1.
    pub const MAX_ANISOTROPY: f32 = 0.95;
    pub const MIN_DEPTH_EXTENT: f32 = 4.0;
    pub const MAX_DEPTH_EXTENT: f32 = 1000.0;
    /// The furthest a height may sit from the origin, either way.
    pub const MAX_HEIGHT: f32 = 100_000.0;
    pub const MIN_RESOLUTION: u32 = 16;
    pub const MAX_RESOLUTION: u32 = 256;
    pub const MIN_SLICES: u32 = 16;
    pub const MAX_SLICES: u32 = 128;
}

impl Default for VolumetricFog {
    /// Off; when turned on, HDRP's defaults: a 400 m mean free path, white albedo,
    /// isotropic, from height 0 thinning to `1/e` at 50 m, reaching 64 m, at the
    /// preset's `Medium` level (`Custom` starts at UE's 160×90×64 grid).
    fn default() -> Self {
        Self {
            active: false,
            mean_free_path: 400.0,
            albedo: [1.0; 3],
            anisotropy: 0.0,
            base_height: 0.0,
            maximum_height: 50.0,
            depth_extent: 64.0,
            global_light_probe_dimmer: 1.0,
            quality: EffectQuality::Medium,
            resolution: 160,
            slices: 64,
        }
    }
}

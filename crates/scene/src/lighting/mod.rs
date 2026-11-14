//! Baked lighting's data for a scene: the lighting sidecar I/O, the irradiance probe
//! grid and its analytic fill, reflection probes, the lightmap pages each mesh
//! wears (`lightmap_set`) and how a page is stored and read back (`page`), an Area
//! light's rectangle (`area`), the baked occlusion culling's PVS and the
//! fingerprints it is checked against (`occlusion`, #1714),
//! the floor plan probe placement reads (`floor`), the scene's bake settings, and
//! the spherical-harmonics basis they all share. The bakes that write them (the
//! lightmapper, the occlusion and probe-placement bakes, mesh maps) are
//! `rusty-lighting`'s.

pub mod area;
#[cfg(feature = "dev")]
pub mod bake_status;
pub mod floor;
pub mod io;
pub mod lightmap_set;
pub mod occlusion;
pub mod page;
pub mod probe;
pub mod probe_fill;
pub mod reflection_probe;
pub mod settings;
pub mod sh;

pub use settings::{DiffuseModel, LightingSettings, MixedLightingMode};

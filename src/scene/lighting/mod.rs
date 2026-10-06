//! Baked lighting for a scene: the lighting sidecar I/O, editor placement of
//! probe volumes, the irradiance probe grid and its analytic fill, reflection
//! probes, the scene's bake settings, and the spherical-harmonics basis they all share.

pub mod io;
pub mod lightmap;
pub mod placement;
pub mod probe;
pub mod probe_fill;
pub mod reflection_probe;
pub mod settings;
pub mod sh;

pub use settings::LightingSettings;

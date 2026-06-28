//! Image-based lighting and environment: cubemap textures and CPU sampling,
//! static-scene cubemap capture, reflection probe baking/prefiltering, the KTX2
//! writer for baked cubemaps, and the skybox renderer.

pub(crate) mod cube_sample;
pub(crate) mod cubemap;
pub(crate) mod cubemap_capture;
pub(crate) mod ktx2_encode;
pub(crate) mod probe_bake;
pub(crate) mod reflection_bake;
pub(crate) mod reflection_prefilter;
pub(crate) mod skybox;

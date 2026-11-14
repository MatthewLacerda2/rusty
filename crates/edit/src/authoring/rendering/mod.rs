//! Rendering authoring ops (Unity's *Add Component › Rendering*): the camera, the
//! light, the LOD group, the Light Probe Proxy Volume, the mesh's subdivisions and
//! the camera's post-processing.

pub mod camera;
pub mod light;
pub mod light_probe_proxy_volume;
pub mod lod_group;
pub mod mesh;
pub mod post_processing;

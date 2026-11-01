//! Rendering components (Unity's *Add Component › Rendering*): the camera, the
//! light, the LOD group, the Light Probe Proxy Volume and the camera's
//! post-processing. `mesh` and `material`
//! keep their own folders beside this one.

pub mod camera;
pub mod light;
pub mod light_probe_proxy_volume;
pub mod lod_group;
pub mod post_processing;

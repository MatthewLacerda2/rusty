//! Shadowmask in the frame (#1695): a `Mixed` lamp under a scene on `Shadowmask`
//! lights a static floor as a `Baked` one does, glints on a glossy one, and its
//! baked static shadow blocks both; a dynamic cube under it still casts a realtime
//! shadow and is shaded by the probes' occlusion behind baked cover; and a scene
//! back on `BakedIndirect` draws exactly what one baked that way draws. Static
//! casters leave a masked light's maps only while its masks are bound and no fog
//! is on.

mod dynamic_tests;
mod edge_tests;
mod fixture;
mod fog_tests;
mod static_tests;
mod statics_tests;

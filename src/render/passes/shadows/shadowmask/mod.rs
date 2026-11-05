//! Shadowmask in the frame (#1695): a `Mixed` lamp under a scene on `Shadowmask`
//! lights a static floor as a `Baked` one does, glints on a glossy one, and its
//! baked static shadow blocks both; a dynamic cube under it still casts a realtime
//! shadow and is shaded by the probes' occlusion behind baked cover; and a scene
//! back on `BakedIndirect` draws exactly what one baked that way draws.

mod dynamic_tests;
mod edge_tests;
mod fixture;
mod static_tests;

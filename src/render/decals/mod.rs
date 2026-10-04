//! src/render/decals/ — clustered surface decals (#638).
//!
//! A decal changes the surface it lands on **before** that surface is lit: its
//! albedo, normal, metallic, roughness and occlusion, each with its own blend
//! weight (`DecalBlend`). So a bullet hole's rim catches the muzzle flash, blood
//! reads wet under a flashlight, and a scorch mark sits in its wall's shadow.
//!
//! The shape is Doom 2016's clustered decals, on the forward+ light grid (#434):
//! - once per frame every decal becomes a [`GpuDecal`] in one storage array, its
//!   maps loaded into one texture array (`atlas`), so no decal owns a buffer or a
//!   bind group (`gpu`);
//! - each camera bins the decals' bounding spheres into its clusters beside its
//!   lights, into the same range and index buffers (`clusters`);
//! - `fs_main` (`shader.wgsl`) walks its cluster's decals, oldest first, and folds
//!   each into the material inputs before any light is summed (`apply_decal` in
//!   `common.wgsl`). Map samples use gradients taken from the decal's own UVs, so
//!   an edge never falls to the smallest mip, and a surface turned away from the
//!   projector fades out instead of streaking.
//!
//! **Who receives.** A material opts out with `receive_decals = false`; transparent
//! materials, unlit draws and mesh particles never receive. A `DepthOnly` camera
//! (the stacked viewmodel or overlay) bins no decals, so the viewmodel never shows
//! the wall's bullet holes.

mod atlas;
mod gpu;
mod record;

pub(crate) use gpu::DecalBuffers;
#[cfg(test)]
pub(crate) use record::GpuDecal;

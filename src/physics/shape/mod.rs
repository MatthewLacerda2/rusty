//! src/physics/shape/mod.rs — turning a `Collider` into a rapier shape: the
//! builder (`build`), the triangle-mesh shape (`mesh`), its convex
//! decomposition on a dynamic body (`decompose`, chosen per body by `fit`), a
//! convex hull measured by its face planes (`hull`) and the world-space triangle read-back
//! (`triangles`).

pub(super) mod build;
#[cfg(test)]
mod build_tests;
#[cfg(test)]
mod conformance;
mod decompose;
pub(super) mod fit;
#[cfg(test)]
pub(super) mod fixture;
pub(super) mod hull;
#[cfg(test)]
mod hull_tests;
#[cfg(test)]
mod internal_edges_tests;
#[cfg(test)]
mod merge_skip_tests;
pub(super) mod mesh;
#[cfg(test)]
mod mesh_tests;
#[cfg(test)]
mod mirror_tests;
#[cfg(test)]
mod round_bounds_tests;
#[cfg(test)]
mod shared_mesh_tests;
pub(super) mod triangles;

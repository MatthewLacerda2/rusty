//! src/physics/shape/mod.rs — turning a `Collider` into a rapier shape: the
//! builder (`build`), the triangle-mesh shape (`mesh`) and the world-space
//! triangle read-back (`triangles`).

pub(super) mod build;
#[cfg(test)]
mod build_tests;
#[cfg(test)]
mod conformance;
#[cfg(test)]
mod hull_tests;
pub(super) mod mesh;
#[cfg(test)]
mod mesh_tests;
#[cfg(test)]
mod mirror_tests;
#[cfg(test)]
mod round_bounds_tests;
pub(super) mod triangles;

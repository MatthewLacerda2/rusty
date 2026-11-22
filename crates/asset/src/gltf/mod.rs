//! glTF 2.0 import: meshes and materials (`import`), the skeleton (`skin`), the
//! animation clips (`anim`) and the node hierarchy that places the meshes (`nodes`).

pub mod anim;
pub mod import;
mod material;
mod nodes;
pub mod skin;
pub mod uri;

#[cfg(test)]
mod import_tests;
#[cfg(test)]
mod uv2_tests;

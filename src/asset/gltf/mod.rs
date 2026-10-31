//! glTF 2.0 import (#79, #80): meshes and materials (`import`), the skeleton
//! (`skin`), the animation clips that drive it (`anim`) and the node hierarchy
//! that places the meshes (`nodes`, #1083).

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

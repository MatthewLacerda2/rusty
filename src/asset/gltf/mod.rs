//! glTF 2.0 import (#79, #80): meshes and materials (`import`), the skeleton
//! (`skin`) and the animation clips that drive it (`anim`).

pub mod anim;
pub mod import;
mod material;
pub mod skin;
pub mod uri;

#[cfg(test)]
mod import_tests;
#[cfg(test)]
mod uv2_tests;

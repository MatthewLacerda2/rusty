//! glTF 2.0 import: meshes and materials (`import`), the skeleton (`skin`), the
//! animation clips (`anim`) and the node hierarchy that places the meshes (`nodes`).

mod anim;
mod conform;
pub mod import;
mod material;
mod nodes;
mod skin;
mod topology;
pub mod uri;

#[cfg(test)]
mod tests;

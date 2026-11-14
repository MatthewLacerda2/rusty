//! crates/scene/src/rules/mod.rs — the bounds a stored value is held to.
//!
//! What is finite and in range before a field keeps it: the float rules every
//! authoring setter shares (`sanitize`), and the bounds the base holds a value to
//! where it reads one that skipped the setters (a scene file, a prefab): a camera
//! lens, a joint drive, a mesh's file-read fields, a material's ranges. They sit
//! below `scene::authoring` so the base holds a value without naming the edit
//! verbs, and the authoring ops store through these same functions, so each bound
//! lives once. The `sanitize` lint counts a call to any function here as holding
//! a setter's float (`docs/linting.md`).

pub mod joint;
pub mod lens;
pub mod material;
pub mod mesh;
pub mod sanitize;

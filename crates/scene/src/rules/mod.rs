//! crates/scene/src/rules/mod.rs — the bounds a stored value is held to.
//!
//! What is finite and in range before a field keeps it: the float rules every
//! authoring setter shares (`sanitize`), and the bounds the base holds a value to
//! where it reads one that skipped the setters (a scene file, a prefab): a camera
//! lens, a joint drive, a mesh's file-read fields, a material's ranges. They sit
//! below `edit::authoring` so the base holds a value without naming the edit
//! verbs, and the authoring ops store through these same functions, so each bound
//! lives once. The `sanitize` lint counts a bare `pub fn` here as holding a
//! setter's float only at the parameter positions it holds (its body tests the
//! parameter with `is_finite()`, or hands it to another rule that holds it), and
//! flags one that holds none (`docs/linting.md`).

pub mod joint;
pub mod lens;
pub mod material;
pub mod mesh;
pub mod sanitize;

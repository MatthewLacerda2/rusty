//! Per-component inspector cards for the selected entity: one module per
//! first-class component (transform, render, material, camera, audio, particles,
//! gameplay) plus the shared card chrome, the read-only frame context, the
//! add-component menu, the linked-prefab override card, and the scene-settings panel.

pub mod add;
pub mod audio;
pub mod camera;
pub mod card;
pub mod context;
pub mod gameplay;
pub mod material;
pub mod particles;
pub mod prefab;
pub mod render;
pub mod script_fields;
pub mod settings;
pub mod transform;

//! The solid path's bind-group indices, the one place they are named: every
//! forward, prepass, transparent, outline, overlay, shadow-caster and particle
//! layout, every `set_bind_group` that draws through one, and the surface
//! shaders' declarations read these, and `render::gpu::groups::tests` holds the
//! engine's WGSL `@group` literals equal to them.
//!
//! **Ordered mostly by how often each group changes.** wgpu-core never
//! deduplicates a bind group set with dynamic offsets, and setting one makes the
//! next draw re-flush every group from its index up (its buffer and texture init
//! actions, a backend `set_bind_group`). The per-draw group, whose dynamic offset
//! moves every batch, sits last, so a batch re-flushes it alone; a batch whose
//! material changes re-flushes the material and the scene group above it too. A
//! layout with fewer groups keeps the rule: its per-draw group is last.
//!
//! **The scene group is not at set 1, though it changes less often than the
//! material.** Measured on an NVIDIA Vulkan driver (RTX 2060), the same SPIR-V and
//! pipelines shade a level's forward pass about a quarter slower with the scene
//! group at set 1 (global, scene, material, draw) than with it at 2 or 3; the draw
//! group's set made no difference there. So the material sits at 1 and the scene
//! group at 2: moving them back to pure change-rate order costs that GPU time.

/// The camera, lighting, environment and light clusters: bound once a pass.
pub const GLOBAL: u32 = 0;

/// What one pass produced for the solids (the shadow maps and tiles, SSAO or
/// SSIL, the opaque copy): bound once a pass. A particle draw's scene depth.
pub const SCENE: u32 = 2;

/// A material's maps, samplers and param uniform: bound per batch, and moving
/// only when the material does. A particle's sprite texture.
pub const MATERIAL: u32 = 1;

/// The per-draw group: the draw uniform at a dynamic offset, joints, instances,
/// objects and material records; a shadow caster's casters. Its offset moves
/// every batch, so nothing sits above it.
pub const DRAW: u32 = 3;

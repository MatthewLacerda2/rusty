//! The solid path's bind-group indices, the one place they are named: every
//! forward, prepass, transparent, outline, overlay, shadow-caster and particle
//! layout, every `set_bind_group` that draws through one, and the surface
//! shaders' declarations read these, and `render::gpu::groups::tests` holds the
//! engine's WGSL `@group` literals equal to them.
//!
//! **Ordered by how often each group changes**, slowest first. wgpu-core never
//! deduplicates a bind group set with dynamic offsets, and setting one makes the
//! next draw re-flush every group from its index up (its buffer and texture init
//! actions, a backend `set_bind_group`). The per-draw group, whose dynamic offset
//! moves every batch, sits last, so a batch re-flushes it alone; the material
//! below it re-flushes only when the material changes. A layout with fewer
//! groups keeps the rule: its per-draw group is last.

/// The camera, lighting, environment and light clusters: bound once a pass.
pub const GLOBAL: u32 = 0;

/// What one pass produced for the solids (the shadow maps and tiles, SSAO or
/// SSIL, the opaque copy): bound once a pass. A particle draw's scene depth.
pub const SCENE: u32 = 1;

/// A material's maps, samplers and param uniform: bound per batch, and moving
/// only when the material does. A particle's sprite texture.
pub const MATERIAL: u32 = 2;

/// The per-draw group: the draw uniform at a dynamic offset, joints, instances,
/// objects and material records; a shadow caster's casters. Its offset moves
/// every batch, so nothing sits above it.
pub const DRAW: u32 = 3;

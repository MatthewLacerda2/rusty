//! Per-draw group last: its dynamic offset moves every batch, and setting a group
//! re-flushes every group from its index up. Scene sits at 2, material at 1, though
//! the scene changes less often: measured on an NVIDIA Vulkan driver (RTX 2060), the
//! forward pass shades about a quarter slower with scene at set 1.
//! `render::gpu::groups::tests` holds the engine's WGSL `@group` literals equal to these.

pub const GLOBAL: u32 = 0;

pub const SCENE: u32 = 2;

pub const MATERIAL: u32 = 1;

pub const DRAW: u32 = 3;

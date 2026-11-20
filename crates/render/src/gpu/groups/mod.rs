//! The solid path's bind groups in index order: the indices are
//! `rusty_shadergen::groups`' (re-exported here), and every solid-path pipeline
//! layout is built through [`Groups::layout`], which places each group at its
//! index, so the order is one edit there and no layout names a number.
//!
//! The order is by how often a group changes (the per-draw group, whose dynamic
//! offset moves every batch, last; the material below it): wgpu re-flushes every
//! group from the lowest one set up to the last at the next draw.

pub(crate) use rusty_shadergen::groups::{DRAW, GLOBAL, MATERIAL, SCENE};

/// How many groups a solid-path layout spans.
pub(crate) const COUNT: usize = DRAW as usize + 1;

/// A solid-path layout's groups by role; one it lacks is a gap in the layout,
/// which a pass need not bind.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Groups<T> {
    /// The camera, lighting and environment ([`GLOBAL`]).
    pub global: T,
    /// What a pass produced for the solids ([`SCENE`]).
    pub scene: Option<T>,
    /// A material's maps ([`MATERIAL`]).
    pub material: Option<T>,
    /// The per-draw group ([`DRAW`]).
    pub draw: T,
}

impl<T: Copy> Groups<T> {
    /// The groups at their indices.
    pub(crate) fn ordered(&self) -> [Option<T>; COUNT] {
        let mut at = [None; COUNT];
        at[GLOBAL as usize] = Some(self.global);
        at[SCENE as usize] = self.scene;
        at[MATERIAL as usize] = self.material;
        at[DRAW as usize] = Some(self.draw);
        at
    }
}

impl Groups<&wgpu::BindGroupLayout> {
    /// The pipeline layout of these groups, labelled `label`.
    pub(crate) fn layout(&self, device: &wgpu::Device, label: &str) -> wgpu::PipelineLayout {
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts: &self.ordered(),
            immediate_size: 0,
        })
    }
}

/// A layout of one group, at the per-draw index: a GPU emitter's step, which
/// shares its declarations with the emitter's draw group.
pub(crate) fn draw_only<T: Copy>(draw: T) -> [Option<T>; COUNT] {
    let mut at = [None; COUNT];
    at[DRAW as usize] = Some(draw);
    at
}

#[cfg(test)]
mod tests;

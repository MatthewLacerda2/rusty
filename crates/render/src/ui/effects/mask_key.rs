//! crates/render/src/ui/effects/mask_key.rs — what a Mask's coverage texture is
//! drawn from (#2354): the key `mask_gpu` compares to skip a still Mask's draw.

use super::BatchUniform;

/// Everything a Mask's coverage texture is drawn from, borrowed from this frame's
/// mesh; equal inputs draw equal texels, so a target holding the last draw's
/// inputs needs no new one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MaskInputs<'a> {
    /// Its own triangles, as the vertex buffer holds them.
    pub(super) vertices: &'a [u8],
    /// Its slot of the batch uniform (its parents' clip).
    pub(super) slot: BatchUniform,
    /// The texture its graphic samples (`None`: the white one).
    pub(super) source: Option<&'a wgpu::TextureView>,
    /// Its parent Mask's draw stamp, whose texture it samples.
    pub(super) parent: Option<u64>,
}

/// [`MaskInputs`] as a target keeps them, past the frame they were read in.
#[derive(Default)]
pub(super) struct DrawnInputs {
    vertices: Vec<u8>,
    slot: Option<BatchUniform>,
    source: Option<wgpu::TextureView>,
    parent: Option<u64>,
}

impl DrawnInputs {
    /// Whether `i` would draw what this holds.
    pub(super) fn matches(&self, i: &MaskInputs) -> bool {
        self.vertices == i.vertices
            && self.slot == Some(i.slot)
            && self.source.as_ref() == i.source
            && self.parent == i.parent
    }

    /// Hold `i`, reusing the vertex allocation.
    pub(super) fn hold(&mut self, i: &MaskInputs) {
        self.vertices.clear();
        self.vertices.extend_from_slice(i.vertices);
        (self.slot, self.source, self.parent) = (Some(i.slot), i.source.cloned(), i.parent);
    }
}

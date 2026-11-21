use crate::ui::effects::BatchUniform;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MaskInputs<'a> {
    pub(super) vertices: &'a [u8],
    pub(super) slot: BatchUniform,
    pub(super) source: Option<&'a wgpu::TextureView>,
    pub(super) parent: Option<u64>,
}

#[derive(Default)]
pub(super) struct DrawnInputs {
    vertices: Vec<u8>,
    slot: Option<BatchUniform>,
    source: Option<wgpu::TextureView>,
    parent: Option<u64>,
}

impl DrawnInputs {
    pub(super) fn matches(&self, i: &MaskInputs) -> bool {
        self.vertices == i.vertices
            && self.slot == Some(i.slot)
            && self.source.as_ref() == i.source
            && self.parent == i.parent
    }

    pub(super) fn hold(&mut self, i: &MaskInputs) {
        self.vertices.clear();
        self.vertices.extend_from_slice(i.vertices);
        (self.slot, self.source, self.parent) = (Some(i.slot), i.source.cloned(), i.parent);
    }
}

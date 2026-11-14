//! A camera's opaque batches as a type of their own (#1963), so the compiler
//! holds the one rule about drawing them.

use super::DrawBatch;

/// A camera's opaque and cutout batches, in draw order. They are drawn only
/// through `Renderer::draw_solids`, which draws what GPU occlusion culling kept and
/// keeps the indirect args the primed depth was built from. The type hands out no
/// slice, so `Renderer::draw_batches` (the translucents' path, which takes one)
/// cannot be given them: a pass that did would draw what the cull hid, or break the
/// forward pass's primed `Equal` depth test, and pass every other gate.
#[derive(Default)]
pub(crate) struct OpaqueBatches(Vec<DrawBatch>);

impl OpaqueBatches {
    pub(super) fn new(batches: Vec<DrawBatch>) -> Self {
        Self(batches)
    }

    /// The batches in draw order; an iterator, never a slice (see the type).
    pub(crate) fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = &DrawBatch> + DoubleEndedIterator + Clone + '_ {
        self.0.iter() // behind `impl Iterator`: no `as_slice`
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn last(&self) -> Option<&DrawBatch> {
        self.0.last()
    }
}

impl std::ops::Index<usize> for OpaqueBatches {
    type Output = DrawBatch;

    fn index(&self, i: usize) -> &DrawBatch {
        &self.0[i]
    }
}

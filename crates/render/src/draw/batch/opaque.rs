use super::{DrawBatch, OpaqueOrder};

#[derive(Default)]
pub(crate) struct OpaqueBatches(Vec<DrawBatch>, OpaqueOrder);

impl OpaqueBatches {
    pub(super) fn new(batches: Vec<DrawBatch>, order: OpaqueOrder) -> Self {
        Self(batches, order)
    }

    /// Whether they were sorted for a forward pass `primed` or not: none to order
    /// is either.
    pub(crate) fn ranked_for(&self, primed: bool) -> bool {
        self.0.is_empty() || self.1 == OpaqueOrder::primed(primed)
    }

    /// The batches in draw order, never a slice: `draw_batches` takes one and would draw what the
    /// cull hid.
    pub(crate) fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = &DrawBatch> + DoubleEndedIterator + Clone + '_ {
        self.0.iter()
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

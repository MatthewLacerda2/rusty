//! What the CPU reads of a draw's material record (#1706): the record itself lives in
//! the renderer's material table on the GPU, and a batch key names its slot, so the
//! few terms a pass decides by on the CPU ride the key beside it.

use crate::render::gpu::pipelines::transparent::TransparentBlend;
use crate::render::MaterialData;

/// The record terms a pass reads on the CPU: whether the draw is lit (the depth
/// prepass and priming, #436, #1499), the blend state its surface asks for, whether
/// it reads the opaque copy or traces SSR (#1523, #1570), whether it writes its
/// depth after drawing, and its motion (#1845). A library edit that moves one of these re-derives the
/// material's users (`render::scene::records::rekeys`); one that moves only other
/// record terms rewrites the record alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RecordTraits {
    pub lit: bool,
    pub blend: TransparentBlend,
    pub uses_opaque: bool,
    pub receive_ssr: bool,
    pub depth_postpass: bool,
    /// Whether it writes its own motion vectors (#1845), in the depth postpass.
    pub writes_motion: bool,
}

impl RecordTraits {
    /// The terms `record` holds.
    pub(crate) fn of(record: &MaterialData) -> Self {
        let s = &record.surface;
        Self {
            lit: record.is_lit != 0,
            blend: TransparentBlend::of(s),
            uses_opaque: s.uses_opaque == 1,
            receive_ssr: s.receive_ssr == 1,
            depth_postpass: s.depth_postpass == 1,
            writes_motion: s.writes_motion == 1,
        }
    }
}

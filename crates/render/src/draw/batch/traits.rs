use crate::gpu::pipelines::transparent::TransparentBlend;
use crate::MaterialData;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RecordTraits {
    pub lit: bool,
    pub blend: TransparentBlend,
    pub uses_opaque: bool,
    pub receive_ssr: bool,
    pub depth_postpass: bool,
    pub writes_motion: bool,
}

impl RecordTraits {
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

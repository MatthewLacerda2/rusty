//! src/render/stats/uploads.rs — the scene assets a frame uploaded (#2123): what a
//! load's warm-up (`render::warm`) moves off the new scene's first frame.

/// Mesh buffers, texture files and lightmap page arrays made since the last frame
/// closed its counters. Rewriting a resident mesh's buffers in place counts too.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Uploads {
    /// How many assets were uploaded.
    pub count: u32,
    /// Their bytes: a mesh's vertex and index data, a texture's or a page array's
    /// GPU size with every mip.
    pub bytes: u64,
}

impl Uploads {
    /// Count one upload of `bytes`.
    pub(crate) fn add(&mut self, bytes: u64) {
        self.count += 1;
        self.bytes += bytes;
    }

    /// Count every upload `other` counted too.
    pub(crate) fn add_all(&mut self, other: Self) {
        self.count += other.count;
        self.bytes += other.bytes;
    }

    /// The counters under their `FrameStats` metric names.
    pub fn pairs(&self) -> [(&'static str, u64); 2] {
        [
            ("asset_uploads", self.count.into()),
            ("asset_upload_bytes", self.bytes),
        ]
    }
}

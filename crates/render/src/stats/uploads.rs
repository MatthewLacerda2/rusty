#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Uploads {
    pub count: u32,
    pub bytes: u64,
}

impl Uploads {
    pub(crate) fn add(&mut self, bytes: u64) {
        self.count += 1;
        self.bytes += bytes;
    }

    pub(crate) fn add_all(&mut self, other: Self) {
        self.count += other.count;
        self.bytes += other.bytes;
    }

    pub fn pairs(&self) -> [(&'static str, u64); 2] {
        [
            ("asset_uploads", self.count.into()),
            ("asset_upload_bytes", self.bytes),
        ]
    }
}

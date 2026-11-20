//! The bake files this world read and did not use, and why: a refused one is what
//! a save rewrites or removes (`BakeFiles::store`), and each is what `make
//! bench-scenes` names as baked data the scene ran without
//! (`dev::bench::scenes::notes::bakes_unused`).

use std::path::Path;

use super::disk::{Disk, Stamp};
use super::record::Mark;
use super::BakeFiles;

impl BakeFiles {
    /// Record that the file at `path` was read and refused (`bytes` did not
    /// decode, for the reason `why`): the next save rewrites it with this world's
    /// bake or removes it, while it is still that file.
    pub fn refused(&self, path: &Path, bytes: &[u8], why: &str) {
        let disk = Disk::File(Stamp::of(bytes));
        let why = Some(format!("refused ({why})"));
        self.record(path, disk, None, None, Mark::Refused, why);
    }

    /// Why this world's last read of the file at `path` did not use it (`"refused
    /// (…)"`, `"could not be read (…)"`), until a later read, write or bake of that
    /// path; `None` when it was used, missing, or never read.
    pub fn not_used(&self, path: &Path) -> Option<String> {
        let seen = self.seen.borrow();
        seen.get(&super::disk::key(path))?.why.clone()
    }
}

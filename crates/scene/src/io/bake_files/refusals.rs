//! The bake files this world read and refused, and why: what a save rewrites or
//! removes (`BakeFiles::store`), and what `make bench-scenes` names as baked data
//! the scene ran without (`dev::bench::scenes::notes::bakes_unused`).

use std::path::{Path, PathBuf};

use super::disk::{key, Disk, Stamp};
use super::record::Mark;
use super::BakeFiles;

impl BakeFiles {
    /// Record that the file at `path` was read and refused (`bytes` did not
    /// decode, for the reason `why`): the next save rewrites it with this world's
    /// bake or removes it, while it is still that file.
    pub fn refused(&self, path: &Path, bytes: &[u8], why: &str) {
        let disk = Disk::File(Stamp::of(bytes));
        self.record(path, disk, None, None, Mark::Refused);
        if let Some(seen) = self.seen.borrow_mut().get_mut(&key(path)) {
            seen.why = Some(why.to_string());
        }
    }

    /// The files this world refused and has not written or read since, each with
    /// why, by path.
    pub fn refusals(&self) -> Vec<(PathBuf, String)> {
        let seen = self.seen.borrow();
        let refused = seen.iter().filter(|(_, s)| s.mark == Mark::Refused);
        refused
            .filter_map(|(path, s)| Some((path.clone(), s.why.clone()?)))
            .collect()
    }
}

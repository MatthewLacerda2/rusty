use std::path::Path;

use super::disk::{Disk, Stamp};
use super::record::{Mark, Seen};
use super::BakeFiles;

impl BakeFiles {
    pub fn refused(&self, path: &Path, bytes: &[u8], why: &str) {
        let disk = Disk::File(Stamp::of(bytes));
        let why = Some(format!("refused ({why})"));
        let seen = Seen::loaded(disk, None, None);
        let mark = Mark::Refused;
        self.record(path, Seen { mark, why, ..seen });
    }

    pub fn not_used(&self, path: &Path) -> Option<String> {
        let seen = self.seen.borrow();
        seen.get(&super::disk::key(path))?.why.clone()
    }
}

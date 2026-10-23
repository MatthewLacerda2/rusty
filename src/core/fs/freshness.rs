//! src/core/fs/freshness.rs — one path-keyed cache's place in the write log, and
//! the files it could not read (#689, #1241).
//!
//! Every cache keyed by a file path has the same two ways to go stale, and keeps
//! them with one [`Freshness`]:
//!
//! - **The engine re-wrote a file** ([`super::written`]): [`Freshness::poll_with`] drains
//!   the log once per frame (or tick) and hands back the paths written since.
//! - **A file was missing when first read.** The cache records a *miss*
//!   ([`Freshness::miss`]) instead of a default under its path, and every
//!   [`MISS_RETRY_FRAMES`] polls the misses whose file has appeared
//!   (`Path::exists`, one per missing path) are handed back beside the written
//!   ones. A miss whose file is still absent is not retried (#1240).
//!
//! The cache drops whatever the returned paths name, misses included
//! ([`Freshness::forget_misses_where`]), so their next lookup reads the file again. Its
//! users are the renderer's texture cache (`render::gpu::textures::freshness`,
//! whose misses the decal atlas shares) and the UI's texture-size table
//! (`ui::TextureSizeReader`). The resolved texture-reference cache
//! (`asset::convert::texture_ref`) reads the write log directly and keeps no
//! retry beat: it answers on these callers' (#1269).

use std::collections::BTreeSet;
#[cfg(test)]
use std::path::Path;

use super::written::written_since;
#[cfg(test)]
use super::written::Stale;

/// How many polls apart missing paths are re-checked on disk.
pub const MISS_RETRY_FRAMES: u64 = 60;

/// One cache's freshness: where it is in the write log, and its misses.
#[derive(Debug)]
pub struct Freshness {
    /// The newest write generation already handed back.
    seen: u64,
    /// Paths whose read failed, until their file appears or the engine writes it.
    misses: BTreeSet<String>,
    /// Polls since the misses were last re-checked.
    polls: u64,
}

/// What changed since the last [`Freshness::poll_with`].
#[derive(Debug, Default, PartialEq)]
pub struct Changes {
    /// Paths the engine wrote, then misses whose file has appeared.
    pub stale: Vec<String>,
    /// Whether this poll re-checked the misses (a cache with misses of its own
    /// retries them on the same beat).
    pub retried: bool,
}

impl Default for Freshness {
    fn default() -> Self {
        Self::new()
    }
}

impl Freshness {
    /// A cache that starts at the write log's current generation, with no misses.
    pub fn new() -> Self {
        Self {
            seen: written_since(u64::MAX).0,
            misses: BTreeSet::new(),
            polls: 0,
        }
    }

    /// Drain the write log, and on the retry beat add the misses whose file has
    /// appeared. Allocates nothing when nothing was written and no retry is due.
    #[cfg(test)]
    pub fn poll(&mut self) -> Changes {
        self.poll_with(|p| Path::new(p).exists())
    }

    /// Drain the write log for a cache whose keys are not all file paths: `appeared`
    /// says whether the file a missed key names now exists (the renderer resolves
    /// a texture reference through the import cache, #1130).
    pub fn poll_with(&mut self, appeared: impl Fn(&str) -> bool) -> Changes {
        let (seen, mut stale) = written_since(self.seen);
        self.seen = seen;
        self.polls += 1;
        let retried = self.polls >= MISS_RETRY_FRAMES;
        if retried {
            self.polls = 0;
            stale.extend(self.misses.iter().filter(|p| appeared(p)).cloned());
        }
        Changes { stale, retried }
    }

    /// Remember that `path` could not be read.
    pub fn miss(&mut self, path: &str) {
        self.misses.insert(path.to_string());
    }

    /// Whether `path` is a remembered miss.
    pub fn is_miss(&self, path: &str) -> bool {
        self.misses.contains(path)
    }

    /// How many misses are remembered.
    #[cfg(test)]
    pub fn miss_count(&self) -> usize {
        self.misses.len()
    }

    /// Forget every miss `stale` names. Returns whether one went.
    #[cfg(test)]
    pub fn forget_misses(&mut self, stale: &Stale) -> bool {
        self.forget_misses_where(|p| stale.matches(p))
    }

    /// Forget every miss `is_stale` accepts (a cache whose keys go stale with
    /// more than their own path). Returns whether one went.
    pub fn forget_misses_where(&mut self, is_stale: impl Fn(&str) -> bool) -> bool {
        let before = self.misses.len();
        self.misses.retain(|p| !is_stale(p));
        self.misses.len() != before
    }

    /// Keep only the misses `keep` accepts (a cache whose keys went away).
    pub fn retain_misses(&mut self, keep: impl FnMut(&String) -> bool) {
        self.misses.retain(keep);
    }
}

#[cfg(test)]
#[path = "freshness_tests.rs"]
mod tests;

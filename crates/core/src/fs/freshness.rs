//! A path-keyed cache goes stale two ways: the engine re-wrote a file (the write log,
//! drained by [`Freshness::poll_with`]), or a file was missing when first read. A miss is
//! recorded instead of a default, and every [`MISS_RETRY_FRAMES`] polls those whose file
//! has appeared are handed back. A miss whose file is still absent is not retried.

use std::collections::BTreeSet;
#[cfg(test)]
use std::path::Path;

use super::written::written_since;
#[cfg(test)]
use super::written::Stale;

/// How many polls apart missing paths are re-checked on disk.
pub const MISS_RETRY_FRAMES: u64 = 60;

#[derive(Debug)]
pub struct Freshness {
    seen: u64,
    misses: BTreeSet<String>,
    polls: u64,
}

#[derive(Debug, Default, PartialEq)]
pub struct Changes {
    pub stale: Vec<String>,
    pub retried: bool,
}

impl Default for Freshness {
    fn default() -> Self {
        Self::new()
    }
}

impl Freshness {
    pub fn new() -> Self {
        Self {
            seen: written_since(u64::MAX).0,
            misses: BTreeSet::new(),
            polls: 0,
        }
    }

    #[cfg(test)]
    pub fn poll(&mut self) -> Changes {
        self.poll_with(|p| Path::new(p).exists())
    }

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

    pub fn miss(&mut self, path: &str) {
        self.misses.insert(path.to_string());
    }

    pub fn is_miss(&self, path: &str) -> bool {
        self.misses.contains(path)
    }

    #[cfg(test)]
    pub fn miss_count(&self) -> usize {
        self.misses.len()
    }

    #[cfg(test)]
    pub fn forget_misses(&mut self, stale: &Stale) -> bool {
        self.forget_misses_where(|p| stale.matches(p))
    }

    pub fn forget_misses_where(&mut self, is_stale: impl Fn(&str) -> bool) -> bool {
        let before = self.misses.len();
        self.misses.retain(|p| !is_stale(p));
        self.misses.len() != before
    }

    pub fn retain_misses(&mut self, keep: impl FnMut(&String) -> bool) {
        self.misses.retain(keep);
    }
}

#[cfg(test)]
#[path = "freshness_tests.rs"]
mod tests;

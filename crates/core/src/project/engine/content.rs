//! crates/core/src/project/engine/content.rs — which engine content a run used.
//!
//! A built binary reads `engine/` (shaders, fonts, bundled scripts) at run time,
//! from [`engine_dir`](crate::project::engine_dir): in a dev build, the checkout it
//! was built in. Check that checkout out at another commit, or keep a binary across
//! a pull, and the binary runs its own code with the other tree's shaders. So a
//! `dev` binary is stamped with the `digest` of the `engine/` it was built with
//! (the root package's `build.rs`, `RUSTY_ENGINE_CONTENT`), registers it at start-up
//! ([`register_built`]), and this module compares it with the folder on disk:
//! [`register_built`] warns once when they differ, and every run report
//! (`results.json`, `bench.json`) carries the [`EngineStamp`].
//!
//! A warning, never a refusal: a cargo build re-stamps on any change under
//! `engine/` (`cargo:rerun-if-changed`), so `cargo run` and `make` never warn, and
//! the editor's shader hot reload follows edits made after start-up without a
//! second check. What warns is a binary run directly against a tree it was not
//! built from, which is the case that lies.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::commit::engine_commit;
use super::digest::digest;

/// The content this binary was built with, once [`register_built`] named it.
static BUILT: OnceLock<String> = OnceLock::new();

/// Which engine a run used: its commit, and the engine content it was built with
/// against what the folder held. `None` fields could not be told (a binary with no
/// stamp: a test, a release build; a folder that could not be read).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineStamp {
    /// [`engine_commit`]: a debug build's is the checkout's at run time.
    pub commit: String,
    /// The `engine/` digest the binary was built with.
    pub content_built: Option<String>,
    /// The digest of the `engine/` folder this run read.
    pub content_found: Option<String>,
    /// Whether the two agree; `None` when either is unknown.
    pub content_matches: Option<bool>,
}

impl EngineStamp {
    /// The stamp of `built` against `found`, at this build's commit.
    pub fn of(built: Option<&str>, found: Option<&str>) -> Self {
        Self {
            commit: engine_commit().to_string(),
            content_built: built.map(str::to_string),
            content_found: found.map(str::to_string),
            content_matches: built.zip(found).map(|(b, f)| b == f),
        }
    }

    /// The one warning a mismatch gets, naming both digests and the folder;
    /// `None` when they match or either is unknown.
    pub fn warning(&self) -> Option<String> {
        if self.content_matches != Some(false) {
            return None;
        }
        Some(format!(
            "this binary was built with engine content {}, but {} holds {}: \
             its shaders and engine files are not the ones this binary was built with \
             (another commit checked out, or files edited since the build). Rebuild it \
             (cargo build; cargo run and make do) before trusting a picture or a timing \
             it makes",
            self.content_built.as_deref()?,
            crate::project::engine_dir().display(),
            self.content_found.as_deref()?,
        ))
    }
}

/// The digest of the `engine/` folder on disk, hashed once per process, when a
/// run first asks (never in a shipped build, which has no run report).
pub fn on_disk() -> Option<&'static str> {
    static FOUND: OnceLock<Option<String>> = OnceLock::new();
    FOUND
        .get_or_init(|| digest(crate::project::engine_dir()))
        .as_deref()
}

/// This run's [`EngineStamp`]: the registered build content against [`on_disk`].
pub fn stamp() -> EngineStamp {
    EngineStamp::of(BUILT.get().map(String::as_str), on_disk())
}

/// Name the content this binary was built with (a `dev` binary's start-up, from
/// its `RUSTY_ENGINE_CONTENT` stamp), and warn once when the folder on disk holds
/// other content. A second call keeps the first.
pub fn register_built(built: &str) {
    if BUILT.set(built.to_string()).is_err() {
        return;
    }
    if let Some(warning) = stamp().warning() {
        log::warn!("[Engine] {warning}");
    }
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;

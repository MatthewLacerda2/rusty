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
//! The folder is hashed once, at a stamped binary's start-up (xxh3 over its few
//! megabytes: milliseconds, even unoptimised); a process with no stamp never
//! hashes it.
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

/// The folder's digest, or why it could not be taken, once [`found`] hashed it.
static FOUND: OnceLock<Result<String, String>> = OnceLock::new();

/// Which engine a run used: its commit, and the engine content it was built with
/// against what the folder held. `None` fields could not be told: a binary with no
/// stamp (a test process, a release build), whose folder is then never hashed, or
/// a folder that could not be hashed (it says why once, at start-up).
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

    /// The one warning a stamped run owes: the content differs (naming both
    /// digests and the folder), or the folder could not be hashed. `None` when they
    /// match or the binary is unstamped.
    pub fn warning(&self) -> Option<String> {
        let built = self.content_built.as_deref()?;
        let engine = crate::project::engine_dir().display();
        let Some(found) = self.content_found.as_deref() else {
            return Some(format!(
                "{engine} could not be hashed, so whether it holds the engine content \
                 this binary was built with ({built}) is unknown"
            ));
        };
        if built == found {
            return None;
        }
        Some(format!(
            "this binary was built with engine content {built}, but {engine} holds {found}: \
             its shaders and engine files are not the ones this binary was built with \
             (another commit checked out, or files edited since the build). Rebuild it \
             (cargo build; cargo run and make do) before trusting a picture or a timing \
             it makes"
        ))
    }
}

/// The digest of the `engine/` folder on disk, or why it could not be taken;
/// hashed once per process, by the first caller ([`register_built`] in a stamped
/// binary, so at start-up).
fn found() -> &'static Result<String, String> {
    FOUND.get_or_init(|| digest(crate::project::engine_dir()))
}

/// The digest of the `engine/` folder on disk, hashed once per process; `None`
/// when it could not be hashed.
pub fn on_disk() -> Option<&'static str> {
    found().as_deref().ok()
}

/// This run's [`EngineStamp`]: the registered build content against [`on_disk`].
/// An unstamped process (a test, a release build) never hashes the folder: with
/// nothing to compare against, its `content_matches` is `None` either way.
pub fn stamp() -> EngineStamp {
    match BUILT.get() {
        Some(built) => EngineStamp::of(Some(built), on_disk()),
        None => EngineStamp::of(None, None),
    }
}

/// Name the content this binary was built with (a `dev` binary's start-up, from
/// its `RUSTY_ENGINE_CONTENT` stamp), hash the folder on disk, and warn once when it
/// holds other content or cannot be hashed. A second call keeps the first.
pub fn register_built(built: &str) {
    if BUILT.set(built.to_string()).is_err() {
        return;
    }
    if let Err(why) = found() {
        log::warn!(
            "[Engine] cannot hash the engine folder ({why}): this run cannot tell whether \
             its shaders and engine files are the ones this binary was built with"
        );
    } else if let Some(warning) = stamp().warning() {
        log::warn!("[Engine] {warning}");
    }
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;

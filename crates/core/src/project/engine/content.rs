use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::commit::engine_commit;
use super::digest::digest;

static BUILT: OnceLock<String> = OnceLock::new();

static FOUND: OnceLock<Result<String, String>> = OnceLock::new();

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineStamp {
    /// [`engine_commit`]: a debug build's is the checkout's at run time.
    pub commit: String,
    pub content_built: Option<String>,
    pub content_found: Option<String>,
    pub content_matches: Option<bool>,
}

impl EngineStamp {
    pub fn of(built: Option<&str>, found: Option<&str>) -> Self {
        Self {
            commit: engine_commit().to_string(),
            content_built: built.map(str::to_string),
            content_found: found.map(str::to_string),
            content_matches: built.zip(found).map(|(b, f)| b == f),
        }
    }

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

fn found() -> &'static Result<String, String> {
    FOUND.get_or_init(|| digest(crate::project::engine_dir()))
}

pub fn on_disk() -> Option<&'static str> {
    found().as_deref().ok()
}

pub fn stamp() -> EngineStamp {
    match BUILT.get() {
        Some(built) => EngineStamp::of(Some(built), on_disk()),
        // Unstamped (a test, a release build): nothing to compare, so never hash the folder.
        None => EngineStamp::of(None, None),
    }
}

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

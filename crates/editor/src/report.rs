//! How an editor action's failure reaches the developer: one console error, as the
//! REPL shows a script's (`Scene.SavePrefab` returns the same error to its caller).
//! The record goes out under the editor's log target, which `rusty-logging` keeps.

use std::fmt::Display;

/// `result`'s value, or `None` after saying in the console that `action` failed.
pub(crate) fn failed<T, E: Display>(action: &str, result: Result<T, E>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(e) => {
            log::error!("[Editor] {action} failed: {e}");
            None
        }
    }
}

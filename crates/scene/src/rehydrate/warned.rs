use std::collections::BTreeSet;
use std::sync::{Mutex, MutexGuard, PoisonError};

// Process-wide: a scene reloads often, on whichever thread, and once is enough.
static WARNED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

fn warned() -> MutexGuard<'static, BTreeSet<String>> {
    WARNED.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) fn warn_once(reference: &str, error: &rusty_asset::ImportError) {
    warn_once_of(reference, || {
        format!("mesh '{reference}' loads empty: {error}")
    });
}

pub fn warn_once_of(key: &str, message: impl FnOnce() -> String) {
    // A quiet stretch leaves the key unspent, for the apply that is heard.
    if rusty_core::quiet::is_quiet() {
        return;
    }
    if warned().insert(key.to_string()) {
        log::warn!("[Scene] {}", message());
    }
}

/// A test hook: the edit side's tests read it.
#[doc(hidden)]
pub fn warned_of(key: &str) -> bool {
    warned().contains(key)
}

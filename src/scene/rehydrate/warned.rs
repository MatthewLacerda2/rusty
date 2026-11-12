//! The warnings a failed rehydrate gives, once per reference (#1295): a scene
//! reloads often, and once is enough, on whichever thread loads it (#2123).

use std::collections::BTreeSet;
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::asset;

/// The references a failed rehydrate was already warned of.
static WARNED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

fn warned() -> MutexGuard<'static, BTreeSet<String>> {
    WARNED.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Say why `reference`'s mesh comes back empty, once per reference, rather than
/// leaving an invisible entity with no explanation.
pub(super) fn warn_once(reference: &str, error: &asset::ImportError) {
    warn_once_of(reference, || {
        format!("mesh '{reference}' loads empty: {error}")
    });
}

/// Say `message` once per `key` in the console, out of the set every failed
/// rehydrate shares: a scene reloads often, and once is enough.
pub(crate) fn warn_once_of(key: &str, message: impl FnOnce() -> String) {
    if warned().insert(key.to_string()) {
        log::warn!("[Scene] {}", message());
    }
}

/// Whether `key` was already warned of ([`warn_once_of`]).
#[cfg(test)]
pub(crate) fn warned_of(key: &str) -> bool {
    warned().contains(key)
}

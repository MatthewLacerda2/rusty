//! src/asset/sidecar/replace.rs — rename a temp file over its target, riding out
//! Windows' transient refusals (#520).
//!
//! On Unix, `rename` replaces the destination even while other threads hold it
//! open. On Windows it fails — access denied or a sharing violation — whenever
//! another handle has the destination open without `FILE_SHARE_DELETE`, and
//! `std::fs::File::open` never asks for that flag. So a writer's rename can lose to
//! a concurrent reader of the same sidecar. The reader closes within microseconds,
//! so the Windows idiom is to retry the rename briefly. Unix never retries: there a
//! `PermissionDenied` is a real permissions problem, not a race.

use std::io;
use std::path::Path;
use std::time::Duration;

/// `ERROR_SHARING_VIOLATION`: another handle has the file open without sharing.
const WINDOWS_SHARING_VIOLATION: i32 = 32;
/// `ERROR_LOCK_VIOLATION`: another process holds a lock on part of the file.
const WINDOWS_LOCK_VIOLATION: i32 = 33;

/// How many times the rename is attempted before its error is reported.
const ATTEMPTS: u32 = 20;
/// Backoff grows linearly by this step (1 ms, 2 ms, …): ~190 ms worst case in total.
const BACKOFF_STEP: Duration = Duration::from_millis(1);

/// Rename `from` over `to`, retrying the transient Windows refusals.
pub(super) fn replace(from: &Path, to: &Path) -> io::Result<()> {
    retry(
        ATTEMPTS,
        BACKOFF_STEP,
        |e| is_transient(e, cfg!(windows)),
        || std::fs::rename(from, to),
    )
}

/// True when `err` is a refusal that clears once another handle closes — only on
/// Windows (`windows`), where an open reader blocks a replacing rename.
pub(super) fn is_transient(err: &io::Error, windows: bool) -> bool {
    windows
        && (err.kind() == io::ErrorKind::PermissionDenied
            || matches!(
                err.raw_os_error(),
                Some(WINDOWS_SHARING_VIOLATION | WINDOWS_LOCK_VIOLATION)
            ))
}

/// Run `op` up to `attempts` times, sleeping `step × n` after the n-th failure, as
/// long as `transient` says the failure is worth waiting out. Returns the first
/// success, the first non-transient error, or the last error once attempts run out.
pub(super) fn retry<T>(
    attempts: u32,
    step: Duration,
    transient: impl Fn(&io::Error) -> bool,
    mut op: impl FnMut() -> io::Result<T>,
) -> io::Result<T> {
    let mut attempt = 1;
    loop {
        match op() {
            Err(e) if attempt < attempts && transient(&e) => {
                std::thread::sleep(step * attempt);
                attempt += 1;
            }
            result => return result,
        }
    }
}

#[cfg(test)]
#[path = "replace_tests.rs"]
mod tests;

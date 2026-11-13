//! crates/core/src/wake.rs — ring the window's frame loop from any thread (#1721).
//!
//! Outside Play the editor sleeps until something can change what it shows. A window
//! event wakes it by itself; work that finishes on another thread (a command arriving
//! on the socket, a file changing on disk, a background import writing its result)
//! does so through [`frame`]. It is only a nudge to draw: it carries no data and the
//! sim never sees it, so it cannot touch determinism. With no window (the headless
//! session, the harness, a test) nothing is installed and [`frame`] does nothing.

use std::sync::OnceLock;

/// What wakes the window's loop. A process runs one event loop, so it is set once.
static BELL: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// Name what [`frame`] rings. The window's event loop calls this before it runs.
pub fn install(bell: impl Fn() + Send + Sync + 'static) {
    let _ = BELL.set(Box::new(bell));
}

/// Ask the window for a frame soon. Call it after the work is visible to the main
/// thread (the message sent, the note taken), so the frame it wakes finds it.
pub fn frame() {
    RINGS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if let Some(bell) = BELL.get() {
        bell();
    }
}

static RINGS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// How many times [`frame`] rang in this process. Tests run side by side, so
/// compare it only as "it went up". A test hook in every build: other crates' tests
/// call it, and a `#[cfg(test)]` item is invisible outside its crate
/// (crates/CLAUDE.md, *Test hooks*).
#[doc(hidden)]
pub fn rings() -> usize {
    RINGS.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn a_ring_reaches_the_installed_bell() {
        let rung = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&rung);
        super::install(move || {
            seen.fetch_add(1, Ordering::Relaxed);
        });
        super::frame();
        assert!(rung.load(Ordering::Relaxed) >= 1);
    }
}

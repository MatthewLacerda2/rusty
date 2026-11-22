//! Rings the window's frame loop from any thread: a nudge to draw that carries no data
//! and that the sim never sees. With no window installed [`frame`] does nothing.

use std::sync::OnceLock;

static BELL: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// Set once, as a process runs one event loop: a later call is ignored.
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

/// A test hook: how many times [`frame`] rang. Tests run side by side, so compare it
/// only as "it went up".
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

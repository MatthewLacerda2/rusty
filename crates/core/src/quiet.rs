//! crates/core/src/quiet.rs — work on this thread whose warnings are not said (#2123).
//!
//! A `Scene.LoadAsync` bakes its navmesh from a staged copy of the document it
//! prepared, and staging applies the document as the activation will again: every
//! authoring warning it raises (`RequireComponent auto-added`, a missing prefab
//! source) would be said twice. [`quietly`] runs such a copy with this thread's
//! engine warnings dropped by the logger (`logging`), and a once-per-key warning
//! (`scene::rehydrate::warn_once_of`) left unspent for the real apply to say. It
//! decides only what is logged, never what is computed.

use std::cell::Cell;

thread_local!(static QUIET: Cell<bool> = const { Cell::new(false) });

/// Run `f` with this thread's warnings unsaid, then put the thread back as it was
/// (a panic included).
pub fn quietly<T>(f: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            QUIET.set(self.0);
        }
    }
    let _restore = Restore(QUIET.replace(true));
    f()
}

/// Whether this thread is inside [`quietly`].
pub fn is_quiet() -> bool {
    QUIET.get()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_work_inside_is_quiet() {
        assert!(!is_quiet());
        let inside = quietly(|| (is_quiet(), quietly(is_quiet), is_quiet()));
        assert_eq!(inside, (true, true, true), "nested stays quiet");
        assert!(!is_quiet(), "and is put back after");
        let other = std::thread::spawn(|| quietly(|| std::thread::spawn(is_quiet).join()));
        assert!(!other.join().unwrap().unwrap(), "another thread is not");
    }
}

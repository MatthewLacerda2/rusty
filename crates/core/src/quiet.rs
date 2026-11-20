//! crates/core/src/quiet.rs — work on this thread whose warnings are not said (#2123).
//!
//! A `Scene.LoadAsync` bakes its navmesh from a staged copy of the document it
//! prepared, and staging applies the document as the activation will again: every
//! authoring warning it raises (`RequireComponent auto-added`, a missing prefab
//! source) would be said twice. [`quietly`] runs such a copy with this thread's
//! engine warnings dropped by the logger (`logging`), and a once-per-key warning
//! (`scene::rehydrate::warn_once_of`) left unspent for the real apply to say. It
//! decides only what is logged, never what is computed.
//!
//! A worker thread carries neither the flag nor a test's per-thread log capture,
//! and its warnings land in whatever order the threads ran. So work spread over
//! threads (`asset::cook::par`) runs each item [`holding`] what it says through
//! [`warn_held!`](crate::warn_held), and the spawning thread [`say`]s each item's
//! warnings after the join, in item order, under its own flag.

use std::cell::{Cell, RefCell};

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

/// One warning [`holding`] kept: its target (the module that raised it) and text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
    pub target: String,
    pub text: String,
}

thread_local!(static HELD: RefCell<Option<Vec<Held>>> = const { RefCell::new(None) });

/// Run `f`, keeping the warnings it raises through [`warn_held!`](crate::warn_held)
/// on this thread instead of logging them, and return them with its result.
/// Nested, the inner call keeps its own; a panic puts the thread back as it was.
pub fn holding<T>(f: impl FnOnce() -> T) -> (T, Vec<Held>) {
    /// What was held before; put back on a panic, or by hand after `f`.
    struct Restore(Option<Option<Vec<Held>>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            if let Some(before) = self.0.take() {
                HELD.set(before);
            }
        }
    }
    let mut restore = Restore(Some(HELD.replace(Some(Vec::new()))));
    let out = f();
    let held = HELD.replace(restore.0.take().flatten());
    (out, held.unwrap_or_default())
}

/// Log `held` on this thread, in order: what [`holding`] kept elsewhere.
pub fn say(held: Vec<Held>) {
    for h in held {
        warn(&h.target, format_args!("{}", h.text));
    }
}

/// A warning from `target`: kept when this thread is [`holding`], logged otherwise.
/// What [`warn_held!`](crate::warn_held) calls.
pub fn warn(target: &str, args: std::fmt::Arguments<'_>) {
    let kept = HELD.with(|h| match h.borrow_mut().as_mut() {
        Some(held) => {
            held.push(Held {
                target: target.to_string(),
                text: args.to_string(),
            });
            true
        }
        None => false,
    });
    if !kept {
        // lint: allow(log-tag) the caller's own text, which opens with its tag
        log::warn!(target: target, "{args}");
    }
}

/// `log::warn!` for code that may run on a worker thread ([`holding`]): the
/// spawning thread says it, in item order.
#[macro_export]
macro_rules! warn_held {
    ($($arg:tt)+) => {
        $crate::quiet::warn(module_path!(), format_args!($($arg)+))
    };
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

    #[test]
    fn a_held_warning_is_kept_and_said_later_in_order() {
        rusty_test_support::logs::init();
        rusty_test_support::logs::take();
        let ((), held) = holding(|| {
            crate::warn_held!("first {}", 1);
            let ((), inner) = holding(|| crate::warn_held!("inner"));
            assert_eq!(inner.len(), 1, "the inner hold keeps its own");
            crate::warn_held!("second");
        });
        assert!(
            rusty_test_support::logs::take().is_empty(),
            "nothing said yet"
        );
        let texts: Vec<&str> = held.iter().map(|h| h.text.as_str()).collect();
        assert_eq!(texts, ["first 1", "second"]);
        assert_eq!(held[0].target, module_path!());
        say(held);
        crate::warn_held!("unheld");
        assert_eq!(
            rusty_test_support::logs::take(),
            ["first 1", "second", "unheld"]
        );
    }
}

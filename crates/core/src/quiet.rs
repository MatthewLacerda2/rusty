//! [`quietly`] drops this thread's engine warnings (and leaves once-per-key warnings
//! unspent), for a staged copy whose warnings the real apply says again. Work spread over
//! threads runs each item [`holding`] its warnings through `warn_held!`, and the
//! spawning thread [`say`]s them after the join, in item order.

use std::cell::{Cell, RefCell};

thread_local!(static QUIET: Cell<bool> = const { Cell::new(false) });

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

pub fn is_quiet() -> bool {
    QUIET.get()
}

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

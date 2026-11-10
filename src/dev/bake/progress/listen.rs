//! Who else hears a bake's progress lines (#1999): the one caller waiting on the
//! verb that bakes. A script verb's log prints each line to stderr; while a
//! listener is set on this thread ([`listening`]), it hears them too. The MCP
//! server listens while its `eval` runs, to send the client
//! `notifications/progress` (a client gives up on a call that says nothing for 30
//! minutes), and the editor's command channel listens while it evaluates a line,
//! to stream the lines to the attached `session-mcp`.
//!
//! Per thread: a bake's log lines are made on the thread that called the verb,
//! the same thread that evaluates the line. One listener at a time; a nested
//! [`listening`] hides the outer one until it returns.

use std::cell::RefCell;

type Listener = Box<dyn FnMut(&str)>;

thread_local! {
    static LISTENER: RefCell<Option<Listener>> = const { RefCell::new(None) };
}

/// Run `body` with `listener` hearing every progress line [`tell`] gets on this
/// thread, then put back whoever listened before.
pub fn listening<R>(listener: impl FnMut(&str) + 'static, body: impl FnOnce() -> R) -> R {
    let outer = LISTENER.with(|l| l.borrow_mut().replace(Box::new(listener)));
    // Put the outer listener back even if `body` unwinds.
    struct Restore(Option<Listener>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let outer = self.0.take();
            LISTENER.with(|l| *l.borrow_mut() = outer);
        }
    }
    let _restore = Restore(outer);
    body()
}

/// Whether a listener hears this thread's progress lines: the attach backend
/// asks the window for them only then.
pub fn is_listening() -> bool {
    LISTENER.with(|l| l.try_borrow().map_or(true, |l| l.is_some()))
}

/// Pass one progress line to this thread's listener, if any. A listener that
/// tells (it would re-enter itself) is not heard twice: the inner line is dropped.
pub fn tell(line: &str) {
    LISTENER.with(|l| {
        if let Ok(mut listener) = l.try_borrow_mut() {
            if let Some(listener) = listener.as_mut() {
                listener(line);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn a_listener_hears_lines_only_while_it_listens() {
        let heard = Rc::new(RefCell::new(Vec::<String>::new()));
        tell("before");
        let h = heard.clone();
        listening(
            move |l| h.borrow_mut().push(l.to_string()),
            || tell("during"),
        );
        tell("after");
        assert_eq!(*heard.borrow(), ["during"]);
        assert!(!is_listening());
        assert!(listening(|_| {}, is_listening));
    }

    #[test]
    fn a_nested_listener_hides_the_outer_one_until_it_returns() {
        let heard = Rc::new(RefCell::new(Vec::<String>::new()));
        let (outer, inner) = (heard.clone(), heard.clone());
        listening(
            move |l| outer.borrow_mut().push(format!("outer {l}")),
            || {
                listening(
                    move |l| inner.borrow_mut().push(format!("inner {l}")),
                    || tell("a"),
                );
                tell("b");
            },
        );
        assert_eq!(*heard.borrow(), ["inner a", "outer b"]);
    }
}

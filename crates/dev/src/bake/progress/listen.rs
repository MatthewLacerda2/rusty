use std::cell::RefCell;

// Per thread: a bake's lines are made on the thread that evaluates the verb's line.
type Listener = Box<dyn FnMut(&str)>;

thread_local! {
    static LISTENER: RefCell<Option<Listener>> = const { RefCell::new(None) };
}

pub fn listening<R>(listener: impl FnMut(&str) + 'static, body: impl FnOnce() -> R) -> R {
    let outer = LISTENER.with(|l| l.borrow_mut().replace(Box::new(listener)));
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

pub fn is_listening() -> bool {
    LISTENER.with(|l| l.try_borrow().map_or(true, |l| l.is_some()))
}

/// A listener that tells re-enters its own borrow, so that inner line is dropped.
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

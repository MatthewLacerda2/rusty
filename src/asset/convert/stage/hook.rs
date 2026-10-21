//! The points between the swap's steps, where a test runs a reader or a second
//! conversion (#1129). Outside tests [`at`] does nothing.

/// A point in one conversion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// The staging folder exists; the converter is about to run.
    Staged,
    /// The textures folder is in place; the old `.glb` still is too.
    TexturesIn,
    /// The new `.glb` is in place, not yet stamped.
    GlbIn,
    /// Stamped; what earlier conversions left is about to be cleared.
    Stamped,
}

#[cfg(test)]
thread_local! {
    static HOOK: std::cell::RefCell<Option<Box<dyn FnMut(Step)>>> =
        const { std::cell::RefCell::new(None) };
}

/// Run `hook` at every [`Step`] of the conversions this thread starts, until the
/// next call. A conversion the hook itself starts runs without it.
#[cfg(test)]
pub fn set(hook: impl FnMut(Step) + 'static) {
    HOOK.with(|h| *h.borrow_mut() = Some(Box::new(hook)));
}

/// Stop running the hook [`set`] installed.
#[cfg(test)]
pub fn unset() {
    HOOK.with(|h| *h.borrow_mut() = None);
}

/// The conversion reached `step`.
pub fn at(step: Step) {
    #[cfg(test)]
    {
        let Some(mut hook) = HOOK.with(|h| h.borrow_mut().take()) else {
            return;
        };
        hook(step);
        HOOK.with(|h| {
            let mut h = h.borrow_mut();
            if h.is_none() {
                *h = Some(hook);
            }
        });
    }
    let _ = step;
}

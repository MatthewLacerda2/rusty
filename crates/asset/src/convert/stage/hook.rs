//! The points between the swap's steps, where a test runs a reader or a second
//! conversion (#1129), and the converter's own I/O, where a test makes it fail
//! (#1172). Outside tests [`at`] and [`io`] do nothing.

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
    /// A converter is about to read its source.
    ReadSource,
    /// A converter is about to extract a texture.
    ExtractTexture,
    /// A converter is about to write its glTF.
    WriteGltf,
}

/// What a test runs at each step.
#[cfg(test)]
type Hook = Box<dyn FnMut(Step)>;

#[cfg(test)]
thread_local! {
    static HOOK: std::cell::RefCell<Option<Hook>> =
        const { std::cell::RefCell::new(None) };
    static FAIL: std::cell::Cell<Option<Step>> = const { std::cell::Cell::new(None) };
}

/// Make the converter I/O at `step` fail on this thread, until `None`.
#[cfg(test)]
pub fn fail_at(step: Option<Step>) {
    FAIL.with(|f| f.set(step));
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

/// A converter's I/O at `step` may go ahead: in a test, an injected failure
/// ([`fail_at`]) when it names `step`.
pub fn io(step: Step) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL.with(std::cell::Cell::get) == Some(step) {
        return Err(std::io::Error::other(format!("injected at {step:?}")));
    }
    let _ = step;
    Ok(())
}

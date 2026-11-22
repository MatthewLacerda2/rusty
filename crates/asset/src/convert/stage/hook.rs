//! Test seams between the swap's steps and in the converter's I/O; no-ops outside tests.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// The staging folder exists; the converter has not run.
    Staged,
    TexturesIn,
    /// The new `.glb` is in place, not yet stamped.
    GlbIn,
    Stamped,
    ReadSource,
    ExtractTexture,
    WriteGltf,
}

#[cfg(test)]
type Hook = Box<dyn FnMut(Step)>;

#[cfg(test)]
thread_local! {
    static HOOK: std::cell::RefCell<Option<Hook>> =
        const { std::cell::RefCell::new(None) };
    static FAIL: std::cell::Cell<Option<Step>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub fn fail_at(step: Option<Step>) {
    FAIL.with(|f| f.set(step));
}

/// A conversion the hook itself starts runs without it.
#[cfg(test)]
pub fn set(hook: impl FnMut(Step) + 'static) {
    HOOK.with(|h| *h.borrow_mut() = Some(Box::new(hook)));
}

#[cfg(test)]
pub fn unset() {
    HOOK.with(|h| *h.borrow_mut() = None);
}

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

pub fn io(step: Step) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL.with(std::cell::Cell::get) == Some(step) {
        return Err(std::io::Error::other(format!("injected at {step:?}")));
    }
    let _ = step;
    Ok(())
}

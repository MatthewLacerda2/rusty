//! A headless run refuses every provider call (#1384).
//!
//! The harness (a scenario, a bench) is a test: CLAUDE.md says a test never makes
//! a provider call, and a scenario drives the world from outside the sim, through
//! the same API a gameplay script gets. Its world only enters Play on its first
//! `Harness.Step`, so the play-state check alone would let a scenario spend before
//! that. A [`HeadlessHold`] lives as long as the harness does, and while one is held
//! on this thread [`edit_mode`] refuses as if the sim were playing.

use std::cell::Cell;

use super::Refusal;

thread_local! {
    static HELD: Cell<u32> = const { Cell::new(0) };
}

/// Held for a headless run's whole life: provider calls on this thread are refused
/// until it drops (see the module doc).
#[derive(Debug)]
pub struct HeadlessHold(());

impl HeadlessHold {
    pub fn take() -> Self {
        HELD.with(|held| held.set(held.get() + 1));
        Self(())
    }
}

impl Drop for HeadlessHold {
    fn drop(&mut self) {
        HELD.with(|held| held.set(held.get().saturating_sub(1)));
    }
}

/// Whether a provider call may go out at all: never during Play (`playing`), never
/// inside a headless run.
pub fn edit_mode(playing: bool) -> Result<(), Refusal> {
    if playing {
        return Err(Refusal::Playing);
    }
    if HELD.with(Cell::get) > 0 {
        return Err(Refusal::Headless);
    }
    Ok(())
}

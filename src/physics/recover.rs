//! src/physics/recover.rs — when the rapier world is thrown away and rebuilt from
//! the scene: rapier's state is derived, the scene is the truth, and a rebuild is
//! [`PhysicsWorld::from_scene`], the path a scene load takes.
//!
//! - **A panic inside rapier's pipeline doesn't end the game** (#1797). Its solver
//!   and island code assume invariants our sync can break (#1775, #1795), and a
//!   panic there is local: in a release build the step catches it, reports the
//!   message in [`PhysicsEvents::rebuilt`] (the caller logs it with the frame), and
//!   rebuilds the world at once, so neither the next tick nor a query between ticks
//!   reads the broken one. What fires on the frame after starts over as after a
//!   scene load: every pair still overlapping or touching enters again, and no pair
//!   exits. In a `debug_assertions` build (tests, `make test`, the proptest) the
//!   panic propagates, so a rebuild never hides a new case.
//! - **A first real step after only zero-length ones runs on a fresh world**
//!   (#1795). rapier bootstraps its persistent islands on the first step with
//!   time in it, renumbering what zero-length steps had already numbered; a body
//!   removed in between leaves an island id past the renumbered table, and the
//!   sleep scan indexes out of bounds. A zero-length step moves nothing, so the
//!   world it leaves is the scene's, and the rebuilt one steps exactly as a world
//!   that never paused. The trigger and contact pairs carry over: a pause doesn't
//!   re-fire an enter.

use super::{PhysicsEvents, PhysicsWorld};
use crate::scene::Scene;

/// What the world remembers to decide a rebuild.
#[derive(Default)]
pub(super) struct Recovery {
    /// Steps rapier has taken: whether any had time in it, and any at all.
    pub stepped: Stepped,
    /// Test-only: catch a panic as a release build does.
    #[cfg(test)]
    pub catch: bool,
    /// Test-only: the next pipeline run panics, the seam the tests force one through.
    #[cfg(test)]
    pub panic_next: bool,
}

/// Which steps rapier has taken since the world was built.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Stepped {
    #[default]
    Never,
    /// Only zero-length ones: the islands are numbered but not bootstrapped.
    Paused,
    /// At least one with time in it.
    Real,
}

impl PhysicsWorld {
    /// Before a step of `dt`: rebuild a world that has only taken zero-length steps
    /// when this one has time in it, and record which kind this is.
    pub(super) fn before_step(&mut self, scene: &Scene, dt: f32) {
        let real = dt > 0.0;
        if real && self.recovery.stepped == Stepped::Paused {
            self.rebuild(scene, true);
        }
        let stepped = &mut self.recovery.stepped;
        *stepped = match (*stepped, real) {
            (_, true) | (Stepped::Real, false) => Stepped::Real,
            (_, false) => Stepped::Paused,
        };
    }

    /// Run rapier's pipeline. In a release build a panic inside it is caught and
    /// its message returned; in a `debug_assertions` build it propagates.
    pub(super) fn run_pipeline(&mut self) -> Result<(), String> {
        if !self.catches_panics() {
            self.pipeline_step();
            return Ok(());
        }
        // AssertUnwindSafe: after a panic the rapier state is thrown away whole by
        // the caller's rebuild; nothing the closure touched is read again.
        let run = std::panic::AssertUnwindSafe(|| self.pipeline_step());
        std::panic::catch_unwind(run).map_err(|payload| panic_message(payload.as_ref()))
    }

    /// The step after a caught panic: rebuild from `scene` at once and report it.
    pub(super) fn recover(&mut self, scene: &Scene, message: String) -> PhysicsEvents {
        self.rebuild(scene, false);
        PhysicsEvents {
            rebuilt: Some(message),
            ..PhysicsEvents::default()
        }
    }

    /// Replace the whole rapier world with one built from `scene`, keeping the
    /// trigger and contact pairs when `keep_pairs`.
    fn rebuild(&mut self, scene: &Scene, keep_pairs: bool) {
        let mut fresh = Self::from_scene(scene);
        if keep_pairs {
            fresh.prev_triggers = std::mem::take(&mut self.prev_triggers);
            fresh.prev_collisions = std::mem::take(&mut self.prev_collisions);
        }
        #[cfg(test)]
        {
            fresh.tracked.always_full = self.tracked.always_full;
            fresh.recovery.catch = self.recovery.catch;
        }
        *self = fresh;
    }

    /// Whether a panic in the pipeline is caught: in a release build, or a test
    /// asking for the release behaviour.
    fn catches_panics(&self) -> bool {
        #[cfg(test)]
        if self.recovery.catch {
            return true;
        }
        !cfg!(debug_assertions)
    }
}

/// A panic payload's text: the `&str` or `String` `panic!` carries.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    let text = payload.downcast_ref::<&str>().map(|s| s.to_string());
    let text = text.or_else(|| payload.downcast_ref::<String>().cloned());
    text.unwrap_or_else(|| "a panic with no message".to_string())
}

#[cfg(test)]
impl PhysicsWorld {
    /// Force a panic in the next step and catch it as a release build does.
    pub(crate) fn force_caught_panic(&mut self) {
        self.recovery.catch = true;
        self.recovery.panic_next = true;
    }
}

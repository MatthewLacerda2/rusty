//! src/physics/recover.rs — the two steps that don't run rapier's pipeline as
//! usual: a zero-length step before the world's first real one, and a step rapier
//! panicked in.
//!
//! - **A zero-length step before the first real one skips the pipeline** (#1795).
//!   rapier bootstraps its persistent islands on the first step with time in it.
//!   A `dt = 0` step returns before that, but its user-change pass has already
//!   given each body a singleton island; a body removed in between frees one of
//!   those ids, the bootstrap renumbers the rest compactly, and the sleep scan
//!   indexes past its table. So until the world has stepped with time in it (Play
//!   starting paused, behind a menu), a zero-length step does what a paused frame
//!   needs and nothing more: the sync, the colliders placed where their bodies are
//!   going so `Physics.Raycast` and the overlap queries see the scene, and the
//!   write-back. It reports no events: no contact is computed while nothing
//!   moves, so a pair touching from the start enters on the first real step, once.
//!   The world it leaves is the one Play built, joints included, so it steps
//!   exactly as a world that never paused.
//! - **A panic inside rapier's pipeline doesn't end the game** (#1797). Its solver
//!   and island code assume invariants our sync can break (#1775, #1795), and a
//!   panic there is local: in a release build the step catches it and rebuilds
//!   the world at once from the scene with [`PhysicsWorld::from_scene`], so
//!   neither the next tick nor a query between ticks reads the broken one, and
//!   reports it in [`PhysicsEvents::rebuilt`] (the caller logs it). The rebuild
//!   keeps what is ours, not rapier's, and was written only by steps that ran:
//!   each joint's built frames (its rest pose and limits stay where they were) and
//!   the trigger and contact pairs, so the next step's enters and exits come from
//!   a real comparison and scripts, which a rebuild keeps, see no enter twice. In
//!   a `debug_assertions` build (tests, `make test`, the proptest) the panic
//!   propagates, so a rebuild never hides a new case. `sync_to_rapier` and
//!   `break_joints` run outside the catch.

use super::watch::Owners;
use super::{PhysicsEvents, PhysicsWorld, Rebuilt};
use crate::scene::Scene;

/// What the world remembers about its own steps.
#[derive(Default)]
pub(super) struct Recovery {
    /// Whether rapier has taken a step with time in it on this world.
    pub bootstrapped: bool,
    /// Steps in a row that rapier panicked in, carried across their rebuilds.
    pub panics_in_a_row: u32,
    /// Test-only: catch a panic as a release build does.
    #[cfg(test)]
    pub catch: bool,
    /// Test-only: the next pipeline run panics, the seam the tests force one through.
    #[cfg(test)]
    pub panic_next: bool,
}

impl PhysicsWorld {
    /// Whether a step of `dt` must skip rapier's pipeline: a zero-length one before
    /// the world's first real step.
    pub(super) fn dormant(&self, dt: f32) -> bool {
        dt <= 0.0 && !self.recovery.bootstrapped
    }

    /// A zero-length step before the first real one, after the sync: place the
    /// synced bodies' colliders for the queries and write the bodies back.
    pub(super) fn dormant_step(&mut self, scene: &mut Scene, synced: &Owners) -> PhysicsEvents {
        self.place_for_queries(synced);
        self.sync_from_rapier(scene, synced, self.awake_owners());
        PhysicsEvents::default()
    }

    /// Move each collider of the `synced` bodies to where its body is going (its
    /// next pose: a kinematic body's target, any other body's pose), as rapier's
    /// user-change pass would, and refit the query tree over the ones that moved.
    fn place_for_queries(&mut self, synced: &Owners) {
        let mut moved = Vec::new();
        for owner in synced.pick(&self.plan).map(|(&owner, _)| owner) {
            let Some(body) = self
                .id_to_body
                .get(&owner)
                .and_then(|&h| self.bodies.get(h))
            else {
                continue;
            };
            let pose = *body.next_position();
            for &handle in body.colliders() {
                let Some(collider) = self.colliders.get_mut(handle) else {
                    continue;
                };
                let Some(&offset) = collider.position_wrt_parent() else {
                    continue;
                };
                if *collider.position() != pose * offset {
                    collider.set_position(pose * offset);
                    moved.push(handle);
                }
            }
        }
        self.refresh_query_tree(&moved);
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
        let mut fresh = Self::from_scene(scene);
        fresh.keep_joint_frames(&self.joints);
        fresh.prev_triggers = std::mem::take(&mut self.prev_triggers);
        fresh.prev_collisions = std::mem::take(&mut self.prev_collisions);
        let in_a_row = self.recovery.panics_in_a_row + 1;
        fresh.recovery.panics_in_a_row = in_a_row;
        #[cfg(test)]
        {
            fresh.tracked.always_full = self.tracked.always_full;
            fresh.recovery.catch = self.recovery.catch;
        }
        *self = fresh;
        PhysicsEvents {
            rebuilt: Some(Rebuilt { message, in_a_row }),
            ..PhysicsEvents::default()
        }
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

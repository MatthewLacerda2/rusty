//! A zero-length step before the first real one must skip rapier's pipeline: its islands bootstrap on the first step with time in it. Only the sync, query placement and write-back run.
//! In release builds a panic in the pipeline is caught and the world rebuilt from the scene; with `debug_assertions` it propagates.

use rapier3d::prelude::Collider;

use super::watch::Owners;
use super::writes::collider_to_write;
use super::{PhysicsEvents, PhysicsWorld, Rebuilt};
use rusty_scene::Scene;

#[derive(Default)]
pub(super) struct Recovery {
    pub bootstrapped: bool,
    pub panics_in_a_row: u32,
    pub catch: bool,
    pub panic_next: bool,
}

impl PhysicsWorld {
    pub(super) fn dormant(&self, dt: f32) -> bool {
        dt <= 0.0 && !self.recovery.bootstrapped
    }

    pub(super) fn dormant_step(&mut self, scene: &mut Scene, synced: &Owners) -> PhysicsEvents {
        self.place_for_queries(synced);
        self.sync_from_rapier(scene, synced, self.awake_owners());
        PhysicsEvents::default()
    }

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
                let differs = |c: &Collider| {
                    c.position_wrt_parent()
                        .is_some_and(|&offset| *c.position() != pose * offset)
                };
                let Some(collider) = collider_to_write(&mut self.colliders, handle, differs) else {
                    continue;
                };
                if let Some(&offset) = collider.position_wrt_parent() {
                    collider.set_position(pose * offset);
                    moved.push(handle);
                }
            }
        }
        self.refresh_query_tree(&moved);
    }

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
            fresh.tracked.always_walk = self.tracked.always_walk;
        }
        fresh.recovery.catch = self.recovery.catch;
        *self = fresh;
        PhysicsEvents {
            rebuilt: Some(Rebuilt { message, in_a_row }),
            ..PhysicsEvents::default()
        }
    }

    fn catches_panics(&self) -> bool {
        self.recovery.catch || !cfg!(debug_assertions)
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    let text = payload.downcast_ref::<&str>().map(|s| s.to_string());
    let text = text.or_else(|| payload.downcast_ref::<String>().cloned());
    text.unwrap_or_else(|| "a panic with no message".to_string())
}

impl PhysicsWorld {
    /// Force a panic in the next step and catch it as a release build does.
    #[doc(hidden)]
    pub fn force_caught_panic(&mut self) {
        self.recovery.catch = true;
        self.recovery.panic_next = true;
    }
}

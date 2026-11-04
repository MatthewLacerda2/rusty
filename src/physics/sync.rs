//! src/physics/sync.rs — pushing the scene into rapier before a step: in full, or
//! for the bodies the World's change log named since the last step (#1707, the
//! table of inputs and channels is `watch`'s).
//!
//! Both paths run the same passes in the same order (plan, joints, enabled,
//! materials, layers, sensors, characters, masses, then each body's state and
//! compound offsets); the incremental one visits the named bodies only, in
//! ascending owner order, the order the full pass visits them in. A body no
//! change named is one the full pass would leave as it is: every setter it calls
//! compares first, and its inputs are what it last pushed.

use std::collections::BTreeSet;

use super::compound::member;
use super::shape::build::body_state;
use super::watch::{carriers, joint_deps, Changed, Owners, Watch};
use super::world::PhysicsWorld;
use crate::scene::Scene;

impl PhysicsWorld {
    /// Push current component state into rapier before stepping: first rebuild
    /// any body whose collider set changed, then every enabled flag (before any
    /// character sweep, so no sweep sees a stale one), then each owner's world
    /// pose and velocities and each compound collider's offset.
    /// Returns the bodies it synced.
    pub(super) fn sync_to_rapier(&mut self, scene: &mut Scene) -> Owners {
        let owners = match self.read_changes(scene) {
            None => {
                self.resync_topology(scene);
                self.tracked.carriers = carriers(scene);
                self.resync_joints_tracked(scene);
                Owners::All
            }
            Some(changed) => self.resync_changed(scene, &changed),
        };
        let matrix_moved = self.tracked.matrix.as_ref() != Some(&scene.collision_matrix);
        if matrix_moved {
            self.tracked.matrix = Some(scene.collision_matrix.clone());
        }
        self.sync_enabled(scene, &owners);
        self.sync_materials(scene, &owners);
        // The matrix is no entity's column: a change re-derives every collider's groups.
        self.sync_layers(scene, if matrix_moved { &Owners::All } else { &owners });
        self.sync_sensors(scene, &owners);
        // Before the masses: a resized capsule changes its body's volume.
        self.sync_characters(scene, &owners);
        self.sync_masses(scene, &owners);
        let plan = std::mem::take(&mut self.plan);
        for (&owner, ids) in owners.pick(&plan) {
            let Some(&handle) = self.id_to_body.get(&owner) else {
                continue;
            };
            let Some(snapshot) = body_state(scene, owner) else {
                continue;
            };
            self.apply_body_state(handle, &snapshot);
            self.sync_compound_parts(scene, owner, ids);
        }
        self.plan = plan;
        owners
    }

    /// The owners of every body rapier has awake: the only ones a step can move.
    pub(super) fn awake_owners(&self) -> BTreeSet<u32> {
        let ids = self.islands.active_bodies();
        ids.filter_map(|h| self.body_to_id.get(&h).copied())
            .collect()
    }

    /// Write integrated poses + velocities back onto the owner entities,
    /// converting each body's world pose into the owner's local `Transform`
    /// (#445). Static and kinematic bodies keep their transform: physics never
    /// moves the one, and the other is where its Transform sent it (no
    /// world↔local round-trip drift). The Transform write refreshes the world
    /// AABB of every collider under the owner, compound children included.
    ///
    /// After a full sync, every owner, ascending. Otherwise only the ones whose
    /// write can differ from what they hold, in the same order: the bodies synced
    /// (an edit may have woken one, or reset a static body's velocity), the ones
    /// awake before or after the step (rapier neither moves a sleeping body nor
    /// wakes one without leaving it awake), and every owner below one whose write
    /// moved it, which the full pass reaches after it and re-reads under its new
    /// parent pose (a body disabled under a falling one holds its world pose).
    pub(super) fn sync_from_rapier(
        &self,
        scene: &mut Scene,
        synced: &Owners,
        mut queue: BTreeSet<u32>,
    ) {
        let Owners::Only(synced) = synced else {
            for &owner in self.plan.keys() {
                self.write_back(scene, owner);
            }
            return;
        };
        queue.extend(synced);
        queue.extend(self.awake_owners());
        while let Some(owner) = queue.pop_first() {
            if !self.write_back(scene, owner) {
                continue;
            }
            let below = scene.world.self_and_descendants(owner).skip(1);
            queue.extend(below.filter(|&d| d > owner && self.plan.contains_key(&d)));
        }
    }

    /// What changed since the last step; `None` for a full pass (the first step,
    /// or a cursor reading `Rebuild`).
    fn read_changes(&mut self, scene: &mut Scene) -> Option<Changed> {
        #[cfg(test)]
        if self.tracked.always_full {
            return None;
        }
        match self.tracked.watch.as_mut() {
            Some(watch) => watch.read(&mut scene.world),
            None => {
                self.tracked.watch = Some(Watch::open(&mut scene.world));
                None
            }
        }
    }

    /// Rebuild what `changed` may have reshaped (the plan, the joints) and return
    /// the bodies to sync: every owner a named id is or belongs to, and every body
    /// just built.
    fn resync_changed(&mut self, scene: &Scene, changed: &Changed) -> Owners {
        let mut owners = BTreeSet::new();
        let replan = self.plan_may_change(scene, changed);
        if replan {
            owners.extend(self.resync_topology(scene));
            self.tracked.carriers = carriers(scene);
        }
        let deps = &self.tracked.joint_deps;
        if replan || changed.joints || changed.all.iter().any(|id| deps.contains(id)) {
            self.resync_joints_tracked(scene);
        }
        for &id in &changed.all {
            if self.plan.contains_key(&id) {
                owners.insert(id);
            }
            owners.extend(self.tracked.member_owner.get(&id));
        }
        Owners::Only(owners)
    }

    /// Whether a named id may have left, joined or moved between bodies: its
    /// collider membership or owner differs from the plan's, or a body carrier
    /// appeared or went (which re-homes every collider below it, named or not).
    fn plan_may_change(&self, scene: &Scene, changed: &Changed) -> bool {
        let t = &self.tracked;
        let w = &scene.world;
        let rehomed = |&id: &u32| member(scene, id) != t.member_owner.get(&id).copied();
        let carrier = |&id: &u32| {
            let carries = w.has_rigidbody(id) || w.has_character_controller(id);
            carries != t.carriers.contains(&id)
        };
        changed.all.iter().any(rehomed) || changed.carriers.iter().any(carrier)
    }

    /// [`Self::resync_joints`], recording what it read for the next step's check.
    fn resync_joints_tracked(&mut self, scene: &Scene) {
        self.resync_joints(scene);
        self.tracked.joint_deps = joint_deps(scene);
    }
}

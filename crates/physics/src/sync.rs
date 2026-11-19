//! crates/physics/src/sync.rs — pushing the scene into rapier before a step: in full, or
//! for the bodies the World's change log named since the last step (#1707, the
//! table of inputs and channels is `watch`'s).
//!
//! Both paths run the same passes in the same order (plan, joints, enabled,
//! materials, layers, sensors, characters, masses, then each body's state and
//! compound offsets), in ascending owner order, the order the full pass visits
//! them in. The incremental one runs each pass over the bodies named on the
//! channels that pass reads (`watch`'s table, [`Named`]: a body that only moved
//! skips the enabled, material, layer, sensor and fit passes, which read no
//! pose, #2344), every pass over a body just built, and the
//! body-state pass also over every body the last step may have changed (rapier
//! had it awake, `Tracked::stepped`): a step changes a body's pose, velocities
//! and sleep, and nothing the other passes read. The solver rewrites the pose of
//! every body in its active set, a static one included (a body just turned
//! static is still there, and its rotation comes back an ulp off what was pushed,
//! #1842), so only a body neither named nor stepped is sure to hold what it was
//! last given; a static one is compared against what was last pushed to it
//! (`Tracked::static_poses`), so neither path re-pushes it for that rounding. A
//! body in neither set is one the full pass would leave as it is: its inputs are
//! what it last pushed, rapier has not touched it since, and no pass so much as
//! borrows a body or collider that already holds what it would write (`writes::body_to_write`, `writes::collider_to_write`).
//! The borrow matters, not only the write: rapier lists whatever is borrowed
//! mutably as modified, and that list's order is the order the next step meets
//! new contact pairs and so solves them (#1798).

use std::collections::BTreeSet;

use super::compound::member;
use super::shape::build::body_state;
use super::watch::{carriers, joint_deps, Changed, Named, Owners, Watch};
use super::world::PhysicsWorld;
use rusty_scene::Scene;

impl PhysicsWorld {
    /// Push current component state into rapier before stepping: first rebuild
    /// any body whose collider set changed, then every enabled flag (before any
    /// character sweep, so no sweep sees a stale one), then each owner's world
    /// pose and velocities and each compound collider's offset.
    /// Returns the bodies whose state it synced.
    pub(super) fn sync_to_rapier(&mut self, scene: &mut Scene) -> Owners {
        let stepped = std::mem::take(&mut self.tracked.stepped);
        let (named, reshaped) = match self.read_changes(scene) {
            None => {
                self.resync_topology(scene);
                self.tracked.carriers = carriers(scene);
                self.resync_joints_tracked(scene);
                #[cfg(test)]
                {
                    self.tracked.visits.changed = 0;
                }
                (Named::all(), None)
            }
            Some(changed) => self.resync_changed(scene, &changed),
        };
        let matrix_moved = self.tracked.matrix.as_ref() != Some(&scene.collision_matrix);
        if matrix_moved {
            self.tracked.matrix = Some(scene.collision_matrix.clone());
        }
        // An asset is no entity's column either: an edit re-applies every material.
        let assets = rusty_scene::physics_material::generation();
        let assets_moved = self.tracked.material_assets != Some(assets);
        self.tracked.material_assets = Some(assets);
        #[cfg(test)]
        self.tracked.visits.record(&named, &self.plan);
        self.sync_enabled(scene, &named.activity);
        let collider = &named.collider;
        self.sync_materials(scene, if assets_moved { &Owners::All } else { collider });
        // The matrix is no entity's column: a change re-derives every collider's groups.
        let layer = if matrix_moved {
            &Owners::All
        } else {
            &named.layer
        };
        self.sync_layers(scene, layer);
        self.sync_sensors(scene, collider);
        // Before the masses: a resized capsule changes its body's volume.
        self.sync_characters(scene, &named.every);
        // A replaced library mesh re-shapes the colliders built from it (#2059),
        // fit to the body's class now, so the refit below finds it current.
        self.sync_mesh_shapes(scene, reshaped.as_ref());
        // A body that changed type re-fits its meshes, cylinders and capsules,
        // before the masses.
        self.sync_fits(scene, &named.fits);
        self.sync_masses(scene, &named.every);
        // The one pass a step's changes reach: pose, velocities, sleep.
        let owners = match named.every {
            Owners::All => Owners::All,
            Owners::Only(mut named) => {
                named.extend(stepped.iter().filter(|o| self.plan.contains_key(o)));
                Owners::Only(named)
            }
        };
        self.sync_states(scene, &owners);
        owners
    }

    /// Each of `owners`' world pose and velocities, and its compound colliders'
    /// offsets.
    fn sync_states(&mut self, scene: &Scene, owners: &Owners) {
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
    /// (an edit may have woken one, or reset a static body's velocity), `queue`
    /// (the ones the step may have changed: awake before or after it), and every
    /// owner below one whose write moved it, which the full pass reaches after it
    /// and re-reads under its new parent pose (a body disabled under a falling one
    /// holds its world pose).
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
    /// the bodies each pass syncs (every owner an id on the pass's channels is or
    /// belongs to, and every body just built), and the colliders whose
    /// library-mesh triangles may have moved.
    fn resync_changed(
        &mut self,
        scene: &Scene,
        changed: &Changed,
    ) -> (Named, Option<BTreeSet<u32>>) {
        #[cfg(test)]
        {
            self.tracked.visits.changed = changed.all.len();
        }
        let replan = self.plan_may_change(scene, changed);
        // A body just built has nothing in rapier to compare against: every pass.
        let built = if replan {
            // Its colliders' triangles were just recorded (`resync_topology`).
            let built = self.resync_topology(scene).into_iter().collect();
            self.tracked.carriers = carriers(scene);
            built
        } else {
            BTreeSet::new()
        };
        let deps = &self.tracked.joint_deps;
        if replan || changed.joints || changed.all.iter().any(|id| deps.contains(id)) {
            self.resync_joints_tracked(scene);
        }
        let owners = |ids: &mut dyn Iterator<Item = &u32>| {
            let mut out = built.clone();
            for id in ids {
                if self.plan.contains_key(id) {
                    out.insert(*id);
                }
                out.extend(self.tracked.member_owner.get(id));
            }
            Owners::Only(out)
        };
        let c = changed;
        let named = Named {
            every: owners(&mut c.all.iter()),
            activity: owners(&mut c.activity.iter()),
            collider: owners(&mut c.collider.iter().chain(&c.character)),
            layer: owners(&mut c.layer.iter()),
            fits: owners(&mut c.collider.iter().chain(c.carriers()).chain(&c.statics)),
        };
        (named, Some(c.reshaped()))
    }

    /// Whether a named id may have left, joined or moved between bodies: its
    /// collider membership or owner differs from the plan's, or a body carrier
    /// appeared or went (which re-homes every collider below it, named or not).
    ///
    /// **Only the ids that can re-home are checked** (`Changed::homes`), so a tick
    /// where only poses moved re-derives no owner. That holds while every way a
    /// parent link, a Collider or a carrier changes stamps one of those channels:
    /// a re-parent, placement or removal stamps the subtree on `Activity`, a
    /// Collider or carrier edit its own column (`ecs::changes::stamp`).
    fn plan_may_change(&self, scene: &Scene, changed: &Changed) -> bool {
        let t = &self.tracked;
        let w = &scene.world;
        let rehomed = |&id: &u32| {
            #[cfg(test)]
            t.visits.members.set(t.visits.members.get() + 1);
            member(scene, id) != t.member_owner.get(&id).copied()
        };
        let carrier = |&id: &u32| {
            let carries = w.has_rigidbody(id) || w.has_character_controller(id);
            carries != t.carriers.contains(&id)
        };
        changed.homes().any(rehomed) || changed.carriers().any(carrier)
    }

    /// [`Self::resync_joints`], recording what it read for the next step's check.
    fn resync_joints_tracked(&mut self, scene: &Scene) {
        self.resync_joints(scene);
        self.tracked.joint_deps = joint_deps(scene);
    }
}

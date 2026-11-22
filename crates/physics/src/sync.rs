//! A borrow alone makes rapier list a body as modified, and that order decides how the next step
//! solves new contact pairs.

use std::collections::BTreeSet;

use super::compound::member;
use super::shape::build::body_state;
use super::watch::{carriers, joint_deps, Changed, Named, Owners, Watch};
use super::world::PhysicsWorld;
use rusty_scene::Scene;

impl PhysicsWorld {
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
        let assets = rusty_scene::physics_material::generation();
        let assets_moved = self.tracked.material_assets != Some(assets);
        self.tracked.material_assets = Some(assets);
        #[cfg(test)]
        self.tracked.visits.record(&named, &self.plan);
        // Before any character sweep, so no sweep sees a stale enabled flag.
        self.sync_enabled(scene, &named.activity);
        let collider = &named.collider;
        self.sync_materials(scene, if assets_moved { &Owners::All } else { collider });
        let layer = if matrix_moved {
            &Owners::All
        } else {
            &named.layer
        };
        self.sync_layers(scene, layer);
        self.sync_sensors(scene, collider);
        self.sync_characters(scene, &named.every);
        self.sync_mesh_shapes(scene, reshaped.as_ref());
        self.sync_fits(scene, &named.fits);
        // After every pass that changes a volume it reads: a sensor, a capsule, a shape.
        self.sync_masses(scene, &named.every);
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

    pub(super) fn awake_owners(&self) -> BTreeSet<u32> {
        let ids = self.islands.active_bodies();
        ids.filter_map(|h| self.body_to_id.get(&h).copied())
            .collect()
    }

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
            // Only later owners: the full pass, ascending, never rewrites an earlier one.
            queue.extend(below.filter(|&d| d > owner && self.plan.contains_key(&d)));
        }
    }

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
        let built = if replan {
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

    /// Only the ids that can re-home are checked (`Changed::homes`), so a tick where only poses
    /// moved re-derives no owner. That holds while every reparent, `Collider` or carrier change
    /// stamps `Activity` or its own column.
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

    fn resync_joints_tracked(&mut self, scene: &Scene) {
        self.resync_joints(scene);
        self.tracked.joint_deps = joint_deps(scene);
    }
}

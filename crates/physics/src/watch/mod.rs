//! rapier's solver rewrites every active body's pose, a body just turned static included, an
//! ulp off what was pushed, and no write-back stamps it. So the sync also visits every body
//! awake around the last step (`Tracked::stepped`), comparing a static one against its last
//! pushed pose (`Tracked::static_poses`), not rapier's: a re-push every step would wake
//! whatever rests on it, for good.

use std::collections::BTreeSet;

use rapier3d::prelude::{Pose, RigidBodyHandle};

use super::compound::{body_owner, BodyPlan};
use rusty_components::{
    CharacterControllerComponent, ColliderComponent, JointComponent, MeshComponent,
    RigidBodyComponent,
};
use rusty_core::collections::Map;
use rusty_ecs::{Activity, Cursor, Layer, StaticSet, World, WorldPose};
use rusty_scene::{CollisionMatrix, Scene};

mod changed;
pub(super) mod mesh_keys;

use changed::take;
pub(super) use changed::{Changed, Named};

#[cfg(test)]
mod borrow_tests;
#[cfg(test)]
mod channel_tests;
#[cfg(test)]
mod edit_tests;
#[cfg(test)]
pub(super) mod fixture_tests;
#[cfg(test)]
mod mesh_keys_tests;
#[cfg(test)]
mod mesh_shape_tests;
#[cfg(test)]
mod ops_tests;
#[cfg(test)]
mod sleep_tests;
#[cfg(test)]
mod stepped_tests;
#[cfg(test)]
mod tests;

pub(super) struct Watch {
    collider: Cursor<ColliderComponent>,
    rigidbody: Cursor<RigidBodyComponent>,
    character: Cursor<CharacterControllerComponent>,
    joint: Cursor<JointComponent>,
    pose: Cursor<WorldPose>,
    activity: Cursor<Activity>,
    layer: Cursor<Layer>,
    statics: Cursor<StaticSet>,
    mesh: Cursor<MeshComponent>,
}

impl Watch {
    pub(super) fn open(world: &mut World) -> Self {
        Self {
            collider: world.change_cursor(),
            rigidbody: world.change_cursor(),
            character: world.change_cursor(),
            joint: world.change_cursor(),
            pose: world.change_cursor(),
            activity: world.change_cursor(),
            layer: world.change_cursor(),
            statics: world.change_cursor(),
            mesh: world.change_cursor(),
        }
    }

    pub(super) fn read(&mut self, world: &mut World) -> Option<Changed> {
        let mut c = Changed::default();
        let (mut joints, mut pose) = (Vec::new(), Vec::new());
        let known = [
            take(world, &mut self.rigidbody, &mut c.rigidbody),
            take(world, &mut self.character, &mut c.character),
            take(world, &mut self.joint, &mut joints),
            take(world, &mut self.collider, &mut c.collider),
            take(world, &mut self.mesh, &mut c.mesh),
            take(world, &mut self.pose, &mut pose),
            take(world, &mut self.activity, &mut c.activity),
            take(world, &mut self.layer, &mut c.layer),
            take(world, &mut self.statics, &mut c.statics),
        ];
        if known.contains(&false) {
            return None;
        }
        let channels = [
            &c.collider,
            &c.rigidbody,
            &c.character,
            &c.activity,
            &c.layer,
            &c.statics,
            &c.mesh,
        ];
        let mut all = pose;
        all.extend(&joints);
        channels.into_iter().for_each(|ids| all.extend(ids));
        all.sort_unstable();
        all.dedup();
        c.all = all;
        c.joints = !joints.is_empty();
        Some(c)
    }
}

pub(super) enum Owners {
    All,
    Only(BTreeSet<u32>),
}

impl Owners {
    pub(super) fn pick<'a>(
        &'a self,
        plan: &'a BodyPlan,
    ) -> Box<dyn Iterator<Item = (&'a u32, &'a Vec<u32>)> + 'a> {
        match self {
            Owners::All => Box::new(plan.iter()),
            Owners::Only(set) => Box::new(set.iter().filter_map(|o| plan.get_key_value(o))),
        }
    }
}

#[derive(Default)]
pub(crate) struct Tracked {
    pub watch: Option<Watch>,
    pub member_owner: Map<u32, u32>,
    pub carriers: BTreeSet<u32>,
    pub joint_deps: BTreeSet<u32>,
    pub stepped: BTreeSet<u32>,
    pub static_poses: Map<RigidBodyHandle, Pose>,
    pub matrix: Option<CollisionMatrix>,
    pub material_assets: Option<u64>,
    pub mesh_keys: mesh_keys::MeshKeys,
    pub follow: Option<crate::follow::Kept>,
    #[cfg(test)]
    pub visits: changed::Visits,
    #[cfg(test)]
    pub always_full: bool,
    #[cfg(test)]
    pub always_walk: bool,
}

impl Tracked {
    pub(super) fn remember_plan(&mut self, plan: &BodyPlan) {
        self.member_owner.clear();
        for (&owner, ids) in plan {
            self.member_owner.extend(ids.iter().map(|&id| (id, owner)));
        }
    }
}

pub(super) fn carriers(scene: &Scene) -> BTreeSet<u32> {
    let w = &scene.world;
    let mut out: BTreeSet<u32> = w.ids_with_rigidbody().into_iter().collect();
    out.extend(w.ids_with_character_controller());
    out
}

pub(super) fn joint_deps(scene: &Scene) -> BTreeSet<u32> {
    let w = &scene.world;
    let mut out = BTreeSet::new();
    for id in w.ids_with_joint() {
        out.insert(id);
        if let Some(other) = w.joint(id).and_then(|j| j.connected_body) {
            out.extend([other, body_owner(scene, other)]);
        }
    }
    out
}

//! src/physics/watch/ — what changed since the last step, read from the World's
//! change log (#1707, on #1705's `ecs::changes`).
//!
//! The sync before each step used to re-derive everything it pushes into rapier
//! from every entity, every tick, to find that nothing changed. It now asks the
//! World which ids were stamped on the channels its inputs live on, and syncs only
//! the bodies those ids belong to. Unity syncs only transforms with `hasChanged`
//! and never re-syncs statics; bevy_rapier and avian sync changed bodies only.
//!
//! **What each sync pass reads, and where its change shows up:**
//!
//! | Pass | Reads | Seen on |
//! |---|---|---|
//! | body plan (`resync_topology`) | a Collider's presence and `active`, a CharacterController's presence, a Rigidbody's presence, the parent chain (`body_carrier`) | `ColliderComponent`, `CharacterControllerComponent`, `RigidBodyComponent`; a re-parent on `WorldPose` (subtree) |
//! | joints (`resync_joints`) | every `Joint`, the joint entity's `activeInHierarchy`, both bodies' owners, handles and `activeInHierarchy` (a disabled body's joint is inert) | `JointComponent`; the joint and connected ids and the connected body's owner on `Activity`, `WorldPose`; a plan change |
//! | `sync_enabled` | `activeInHierarchy` of the owner and each collider | `Activity` |
//! | `sync_materials`, `sync_sensors` | the Collider's material and `is_trigger`, a CharacterController's presence | `ColliderComponent`, `CharacterControllerComponent` |
//! | `sync_layers` | the entity's layer, **and the scene's collision matrix** | `Layer`; the matrix is not an entity column, so it is compared to the copy kept here and a change re-runs the pass over every collider |
//! | `sync_masses` | the Rigidbody's `mass`, each collider's volume and sensor flag (a crouch resizes a capsule) | `RigidBodyComponent`, `ColliderComponent`, `CharacterControllerComponent`, `WorldPose` (scale) |
//! | `sync_characters` | the CharacterController, the owner's and the capsule's world pose | `CharacterControllerComponent`, `WorldPose` |
//! | body state (`apply_body_state`) | the owner's world pose, its Rigidbody (class, velocities, gravity, CCD), its static flag, a CharacterController's presence | `WorldPose`, `RigidBodyComponent`, `StaticSet`, `CharacterControllerComponent` |
//! | compound parts | the owner's and each part's world pose | `WorldPose` |
//!
//! No pass reads a settings value or an asset: gravity is the world's own, and a
//! mesh collider's triangles are read when its body is built, never synced.
//!
//! **Rebuild.** A cursor reading [`Changes::Rebuild`] (the first step, a scene
//! load, Play's restore, an Undo or Redo, a reader that fell behind) runs today's
//! full pass, over every body.
//!
//! **A body's own write-back.** A dynamic body's pose and velocities written back
//! after a step stamp `WorldPose` and `RigidBodyComponent`, so a body that moved is
//! synced again before the next step, exactly as every body was before this log
//! existed; what is skipped is a body nothing moved. Telling the write-back apart
//! from an edit would save the moving bodies' sync too, but a compound part's offset
//! re-derived from a moved owner can round differently, so a moving body would no
//! longer step byte-identically to the full pass. A body at rest writes back the
//! values it holds, and that stamps nothing (`write_back` compares first).

use std::collections::BTreeSet;

use super::compound::{body_owner, BodyPlan};
use crate::components::{
    CharacterControllerComponent, ColliderComponent, JointComponent, RigidBodyComponent,
};
use crate::core::collections::Map;
use crate::ecs::{Activity, Changes, Channel, Cursor, Layer, StaticSet, World, WorldPose};
use crate::scene::{CollisionMatrix, Scene};

#[cfg(test)]
mod edit_tests;
#[cfg(test)]
mod fixture_tests;
#[cfg(test)]
mod ops_tests;
#[cfg(test)]
mod sleep_tests;
#[cfg(test)]
mod tests;

/// The cursors physics reads before each step, one per channel in the table above.
pub(super) struct Watch {
    collider: Cursor<ColliderComponent>,
    rigidbody: Cursor<RigidBodyComponent>,
    character: Cursor<CharacterControllerComponent>,
    joint: Cursor<JointComponent>,
    pose: Cursor<WorldPose>,
    activity: Cursor<Activity>,
    layer: Cursor<Layer>,
    statics: Cursor<StaticSet>,
}

/// What changed since the last read, when the log could say.
#[derive(Debug, Default)]
pub(super) struct Changed {
    /// Every id stamped on any channel, ascending, each once.
    pub all: Vec<u32>,
    /// The ids whose Rigidbody or CharacterController was edited, attached or
    /// detached: a body carrier may have appeared or gone.
    pub carriers: Vec<u32>,
    /// A `Joint` was edited, attached, detached or broken.
    pub joints: bool,
}

/// Read `cursor` into `ids`; `false` when it reads `Rebuild`.
fn take<T: Channel>(world: &mut World, cursor: &mut Cursor<T>, ids: &mut Vec<u32>) -> bool {
    match world.changes(cursor) {
        Changes::Rebuild => false,
        Changes::Ids(found) => {
            ids.extend(found);
            true
        }
    }
}

impl Watch {
    /// Open every cursor: they have read everything up to now.
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
        }
    }

    /// What changed since the last read; `None` when any cursor reads `Rebuild`.
    /// Every cursor is read either way, so each has read everything after this.
    pub(super) fn read(&mut self, world: &mut World) -> Option<Changed> {
        let mut carriers = Vec::new();
        let mut joints = Vec::new();
        let mut rest = Vec::new();
        let known = [
            take(world, &mut self.rigidbody, &mut carriers),
            take(world, &mut self.character, &mut carriers),
            take(world, &mut self.joint, &mut joints),
            take(world, &mut self.collider, &mut rest),
            take(world, &mut self.pose, &mut rest),
            take(world, &mut self.activity, &mut rest),
            take(world, &mut self.layer, &mut rest),
            take(world, &mut self.statics, &mut rest),
        ];
        if known.contains(&false) {
            return None;
        }
        carriers.sort_unstable();
        carriers.dedup();
        let mut all = rest;
        all.extend(&carriers);
        all.extend(&joints);
        all.sort_unstable();
        all.dedup();
        Some(Changed {
            all,
            carriers,
            joints: !joints.is_empty(),
        })
    }
}

/// Which bodies a sync pass visits: every plan owner, or the ones a change named.
pub(super) enum Owners {
    All,
    /// Ascending, so a pass visits them in the plan's own order.
    Only(BTreeSet<u32>),
}

impl Owners {
    /// The plan entries this selection visits, by ascending owner.
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

/// What physics remembers between steps to read the change log against.
#[derive(Default)]
pub(in crate::physics) struct Tracked {
    /// Opened on the first step: that step, with none, is a full pass.
    pub watch: Option<Watch>,
    /// Collider entity → the owner whose body carries it, as the plan says.
    pub member_owner: Map<u32, u32>,
    /// The entities with a Rigidbody or a CharacterController at the last plan.
    pub carriers: BTreeSet<u32>,
    /// Every `Joint` entity, the body each one names and that body's owner, at the
    /// last joint sync.
    pub joint_deps: BTreeSet<u32>,
    /// The collision matrix the colliders' groups were last derived from.
    pub matrix: Option<CollisionMatrix>,
    /// Ignore the log and run the full pass every step: the reference the
    /// equivalence tests compare against.
    #[cfg(test)]
    pub always_full: bool,
}

impl Tracked {
    /// Record `plan`'s member → owner map.
    pub(super) fn remember_plan(&mut self, plan: &BodyPlan) {
        self.member_owner.clear();
        for (&owner, ids) in plan {
            self.member_owner.extend(ids.iter().map(|&id| (id, owner)));
        }
    }
}

/// Every entity that carries a body of its own: a Rigidbody or a CharacterController.
pub(super) fn carriers(scene: &Scene) -> BTreeSet<u32> {
    let w = &scene.world;
    let mut out: BTreeSet<u32> = w.ids_with_rigidbody().into_iter().collect();
    out.extend(w.ids_with_character_controller());
    out
}

/// Every `Joint` entity, the connected body it names and that body's owner: what
/// `resync_joints` reads besides the plan. The owner's `activeInHierarchy` is its
/// body's enabled flag, which decides whether the joint is built (#1775), and it
/// can flip without the connected entity's own (already inactive) flag moving.
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

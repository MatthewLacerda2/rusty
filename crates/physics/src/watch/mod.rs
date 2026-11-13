//! crates/physics/src/watch/ — what changed since the last step, read from the World's
//! change log (#1707, on #1705's `ecs::changes`).
//!
//! The sync before each step used to re-derive everything it pushes into rapier
//! from every entity, every tick, to find that nothing changed. It now asks the
//! World which ids were stamped on the channels its inputs live on, and syncs only
//! the bodies those ids belong to (and, for pose and velocities, the bodies the
//! last step may have changed, below). Unity syncs only transforms with `hasChanged`
//! and never re-syncs statics; bevy_rapier and avian sync changed bodies only.
//!
//! **What each sync pass reads, and where its change shows up:**
//!
//! | Pass | Reads | Seen on |
//! |---|---|---|
//! | body plan (`resync_topology`) | a Collider's presence and `active`, a CharacterController's presence, a Rigidbody's presence, the parent chain (`body_carrier`) | `ColliderComponent`, `CharacterControllerComponent`, `RigidBodyComponent`; a re-parent on `WorldPose` (subtree) |
//! | joints (`resync_joints`) | every `Joint`, the joint entity's `activeInHierarchy`, both bodies' owners, handles and `activeInHierarchy` (a disabled body's joint is inert) | `JointComponent`; the joint and connected ids and the connected body's owner on `Activity`, `WorldPose`; a plan change |
//! | `sync_enabled` | `activeInHierarchy` of the owner and each collider | `Activity` |
//! | `sync_materials`, `sync_sensors` | the Collider's material (inline, or its asset's) and `is_trigger`, a CharacterController's presence | `ColliderComponent`, `CharacterControllerComponent`; an asset is not an entity column, so a move of the assets' `generation` re-runs the material pass over every collider |
//! | `sync_layers` | the entity's layer, **and the scene's collision matrix** | `Layer`; the matrix is not an entity column, so it is compared to the copy kept here and a change re-runs the pass over every collider |
//! | `sync_masses` | the Rigidbody's `mass`, each collider's volume and sensor flag (a crouch resizes a capsule) | `RigidBodyComponent`, `ColliderComponent`, `CharacterControllerComponent`, `WorldPose` (scale) |
//! | `sync_fits` | the owner's body class (as body state reads it), each concave mesh, cylinder and capsule collider's shape and geometry | `RigidBodyComponent`, `StaticSet`, `CharacterControllerComponent`, `ColliderComponent` |
//! | `sync_mesh_shapes` | a mesh collider's library-mesh triangles (#2059, `mesh_keys`) | `ColliderComponent` (a kept copy), `MeshComponent` |
//! | `sync_characters` | the CharacterController, the owner's and the capsule's world pose | `CharacterControllerComponent`, `WorldPose` |
//! | body state (`apply_body_state`) | the owner's world pose, its Rigidbody (class, velocities, gravity, CCD, damping), its static flag, a CharacterController's presence | `WorldPose`, `RigidBodyComponent`, `StaticSet`, `CharacterControllerComponent` |
//! | compound parts | the owner's and each part's world pose | `WorldPose` |
//!
//! No pass reads a settings value, and the one asset a pass reads is a physics
//! material (above): gravity is the world's own, and a mesh collider's triangles
//! are read when its body is built, again when its body changes type
//! (`sync_fits`, #1886), and when they come from a library mesh that was
//! replaced (`sync_mesh_shapes`, #2059), never synced otherwise.
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
//!
//! **What a step changed without a write-back.** rapier's solver rewrites the pose
//! of every body in its active set, not only a dynamic one: a body just turned
//! static is still there, and its rotation comes back an ulp off the one pushed
//! (#1842). No write-back stamps it (physics never moves a static body's
//! Transform), so the log alone would leave it while the full pass, comparing
//! with rapier's pose, would re-push it. The incremental sync therefore also
//! visits every body awake before or after
//! the last step (`Tracked::stepped`) in the body-state pass, the one pass whose
//! inputs a step changes (pose, velocity, sleep). A static body is then compared
//! against the pose last pushed to it (`Tracked::static_poses`), not rapier's, so
//! neither path re-pushes it for the solver's rounding and both keep the solver's
//! value: a re-push every step would wake whatever rests on it, for good.

use std::collections::BTreeSet;

use rapier3d::prelude::{Pose, RigidBodyHandle};

use super::compound::{body_owner, BodyPlan};
use rusty_components::{
    CharacterControllerComponent, ColliderComponent, JointComponent, MeshComponent,
    RigidBodyComponent,
};
use rusty_core::collections::Map;
use rusty_ecs::{Activity, Changes, Channel, Cursor, Layer, StaticSet, World, WorldPose};
use rusty_scene::{CollisionMatrix, Scene};

pub(super) mod mesh_keys;

#[cfg(test)]
mod borrow_tests;
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
    mesh: Cursor<MeshComponent>,
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
    /// The ids whose Collider or Mesh was stamped: a mesh collider's triangles may
    /// have moved (`mesh_keys`). Ascending, each once.
    pub reshaped: Vec<u32>,
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
            mesh: world.change_cursor(),
        }
    }

    /// What changed since the last read; `None` when any cursor reads `Rebuild`.
    /// Every cursor is read either way, so each has read everything after this.
    pub(super) fn read(&mut self, world: &mut World) -> Option<Changed> {
        let mut carriers = Vec::new();
        let mut joints = Vec::new();
        let mut rest = Vec::new();
        let mut reshaped = Vec::new();
        let known = [
            take(world, &mut self.rigidbody, &mut carriers),
            take(world, &mut self.character, &mut carriers),
            take(world, &mut self.joint, &mut joints),
            take(world, &mut self.collider, &mut reshaped),
            take(world, &mut self.mesh, &mut reshaped),
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
        reshaped.sort_unstable();
        reshaped.dedup();
        let mut all = rest;
        all.extend(&reshaped);
        all.extend(&carriers);
        all.extend(&joints);
        all.sort_unstable();
        all.dedup();
        Some(Changed {
            all,
            carriers,
            joints: !joints.is_empty(),
            reshaped,
        })
    }
}

/// Which bodies a sync pass visits: every plan owner, or some of them (the ones a
/// change named, and for the body-state pass and the write-back also the ones
/// the last step may have changed, `Tracked::stepped`).
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
pub(crate) struct Tracked {
    /// Opened on the first step: that step, with none, is a full pass.
    pub watch: Option<Watch>,
    /// Collider entity → the owner whose body carries it, as the plan says.
    pub member_owner: Map<u32, u32>,
    /// The entities with a Rigidbody or a CharacterController at the last plan.
    pub carriers: BTreeSet<u32>,
    /// Every `Joint` entity, the body each one names and that body's owner, at the
    /// last joint sync.
    pub joint_deps: BTreeSet<u32>,
    /// The owners the last step may have changed (awake before or after it), which
    /// the next incremental sync visits whether a change named them or not: rapier's
    /// solver rewrites an awake body's pose, even a static one's (#1842). Empty after
    /// a step that skipped rapier.
    pub stepped: BTreeSet<u32>,
    /// The pose last pushed to each static body, which its next push is compared
    /// against instead of rapier's (`writes::state_differs`): the solver rounds a
    /// static body still in its active set, and a re-push every step would keep
    /// whatever rests on it awake (#1842). Both paths push through
    /// `apply_body_state`, so both read and write the same record.
    pub static_poses: Map<RigidBodyHandle, Pose>,
    /// The collision matrix the colliders' groups were last derived from.
    pub matrix: Option<CollisionMatrix>,
    /// The physics material assets' generation the materials were last synced
    /// against (`scene::physics_material::generation`, #1878).
    pub material_assets: Option<u64>,
    /// The library-mesh triangles each mesh collider's shape was built from.
    pub mesh_keys: mesh_keys::MeshKeys,
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

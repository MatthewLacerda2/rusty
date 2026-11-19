//! crates/physics/src/watch/changed.rs — what the change log named since the last
//! step, kept per channel (#2344), so each sync pass reads only the channels its
//! inputs live on (`watch`'s table), and the owners each pass visits for it (`Named`).

use std::collections::BTreeSet;

use super::Owners;
use rusty_ecs::{Changes, Channel, Cursor, World};

/// What changed since the last read, when the log could say: the ids each channel
/// named, so each pass reads only the channels its inputs live on (`watch`'s
/// table). Every list ascending, each id once.
#[derive(Debug, Default)]
pub(crate) struct Changed {
    /// Every id stamped on any channel: what the body-state, character, mass and
    /// compound passes and the joint check read.
    pub all: Vec<u32>,
    pub collider: Vec<u32>,
    pub rigidbody: Vec<u32>,
    pub character: Vec<u32>,
    pub activity: Vec<u32>,
    pub layer: Vec<u32>,
    pub statics: Vec<u32>,
    pub mesh: Vec<u32>,
    /// A `Joint` was edited, attached, detached or broken.
    pub joints: bool,
}

impl Changed {
    /// The ids whose Rigidbody or CharacterController was edited, attached or
    /// detached: a body carrier may have appeared or gone.
    pub(crate) fn carriers(&self) -> impl Iterator<Item = &u32> {
        self.rigidbody.iter().chain(&self.character)
    }

    /// The ids whose collider membership or owner may have moved ([`member`]'s
    /// inputs): a Collider (its `active`), a carrier, or a parent link, which every
    /// re-parent, placement and removal stamps on `Activity` for the whole subtree
    /// (`World::stamp_hierarchy_below`, `World::touch_entity`). A pure move stamps
    /// none of these, and cannot re-home a collider.
    ///
    /// [`member`]: crate::compound::member
    pub(crate) fn homes(&self) -> impl Iterator<Item = &u32> {
        self.collider
            .iter()
            .chain(self.carriers())
            .chain(&self.activity)
    }

    /// The ids whose Collider or Mesh was stamped: a mesh collider's triangles may
    /// have moved (`mesh_keys`).
    pub(crate) fn reshaped(&self) -> BTreeSet<u32> {
        self.collider.iter().chain(&self.mesh).copied().collect()
    }
}

/// Read `cursor` into `ids`, ascending and each once; `false` when it reads `Rebuild`.
pub(super) fn take<T: Channel>(
    world: &mut World,
    cursor: &mut Cursor<T>,
    ids: &mut Vec<u32>,
) -> bool {
    match world.changes(cursor) {
        Changes::Rebuild => false,
        Changes::Ids(found) => {
            // `Log::since` already returns its ids ascending and each once, and
            // every cursor reads into a list of its own.
            *ids = found;
            true
        }
    }
}

/// The owners each sync pass visits, by the channels its inputs live on (`watch`'s
/// table). The body-state pass adds the stepped bodies to `every` (`sync_to_rapier`).
pub(crate) struct Named {
    /// Any channel: characters, masses, body state and compound parts, which read
    /// a world pose.
    pub every: Owners,
    /// `Activity`: `sync_enabled`.
    pub activity: Owners,
    /// Collider and CharacterController: `sync_materials`, `sync_sensors`.
    pub collider: Owners,
    /// `Layer`: `sync_layers`.
    pub layer: Owners,
    /// Collider, Rigidbody, CharacterController, StaticSet: `sync_fits`.
    pub fits: Owners,
}

impl Named {
    /// Every owner in every pass: the full sync.
    pub(crate) fn all() -> Self {
        Self {
            every: Owners::All,
            activity: Owners::All,
            collider: Owners::All,
            layer: Owners::All,
            fits: Owners::All,
        }
    }
}

/// A test's count of the work the incremental sync did (`watch`'s cost tests):
/// `compound::member` calls made by the plan check, ever, and the owners each
/// pass was handed at the last sync, before the matrix or an asset widened one.
#[cfg(test)]
#[derive(Default, Debug)]
pub(crate) struct Visits {
    pub members: std::cell::Cell<u64>,
    /// The ids any channel named at the last sync: zero after a full one.
    pub changed: usize,
    pub every: usize,
    pub activity: usize,
    pub collider: usize,
    pub layer: usize,
    pub fits: usize,
}

#[cfg(test)]
impl Visits {
    pub(crate) fn record(&mut self, named: &Named, plan: &crate::compound::BodyPlan) {
        let n = |o: &Owners| o.pick(plan).count();
        self.every = n(&named.every);
        self.activity = n(&named.activity);
        self.collider = n(&named.collider);
        self.layer = n(&named.layer);
        self.fits = n(&named.fits);
    }
}

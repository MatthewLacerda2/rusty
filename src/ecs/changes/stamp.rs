//! src/ecs/changes/stamp.rs — where the World stamps its logs (#1705).
//!
//! A column write is [`World::touch`] (or [`World::touch_entity`] for a whole
//! entity): one call that notes the write for the edit journal and stamps the
//! log, so a raw write cannot do one and forget the other. The raw hecs writes
//! live in `world.rs` and `placement.rs` only, behind `World::inner`, which is
//! private to `ecs`; each of them goes through one of these two.
//!
//! A derived channel is stamped at the source of its fact, its subtree included:
//! [`World::stamp_pose_below`] from `World::moved`, [`World::stamp_hierarchy_below`]
//! wherever an `active` flag or a parent link changes. A walk is taken only while a
//! cursor is open on the channel it feeds; with none, an `any` and one stamp keep
//! [`World::static_revision`] moving.

use super::{Activity, Channel, Layer, WorldPose, STATIC};
use crate::ecs::World;

impl World {
    /// A write to column `T` of `id`: noted for the journal and stamped on `T`.
    pub(in crate::ecs) fn touch<T: Channel>(&mut self, id: u32) {
        const { assert!(T::COLUMN, "a derived channel is stamped at its source") };
        self.note(id);
        if self.changes.logs.len() == 1 {
            return; // only `StaticSet` exists: no column was ever opened
        }
        if let Some(at) = self.changes.find::<T>() {
            self.stamp_at(at, id);
        }
    }

    /// `id` written whole (placed, overwritten or removed; `is_static` its flag):
    /// noted for the journal and stamped on every column, [`Activity`], [`Layer`]
    /// and, when static, [`super::StaticSet`]. [`WorldPose`] follows from
    /// `World::moved` (or, for a removal, its own walk), so its stamps are always a
    /// whole subtree's.
    pub(in crate::ecs) fn touch_entity(&mut self, id: u32, is_static: bool) {
        self.note(id);
        if is_static {
            self.stamp_static(id);
        }
        if self.changes.logs.len() == 1 {
            return;
        }
        let live = self.order.len();
        for log in self.changes.logs.iter_mut().filter(|l| l.column) {
            log.stamp(id, live);
        }
        for at in [
            self.changes.find::<Activity>(),
            self.changes.find::<Layer>(),
        ] {
            at.into_iter().for_each(|at| self.stamp_at(at, id));
        }
    }

    fn stamp_at(&mut self, at: usize, id: u32) {
        let live = self.order.len();
        self.changes.logs[at].stamp(id, live);
    }

    /// `id`'s membership of the static set may have changed ([`super::StaticSet`]).
    pub(in crate::ecs) fn stamp_static(&mut self, id: u32) {
        self.stamp_at(STATIC, id);
    }

    /// `id`'s layer changed ([`Layer`]).
    pub(in crate::ecs) fn stamp_layer(&mut self, id: u32) {
        if let Some(at) = self.changes.find::<Layer>() {
            self.stamp_at(at, id);
        }
    }

    /// Whether `id` and everything below it are active in the hierarchy may have
    /// changed (an activation or a re-parent there, a placement that brings
    /// children): stamp the subtree on [`Activity`], and its static entities on
    /// [`super::StaticSet`].
    pub(in crate::ecs) fn stamp_hierarchy_below(&mut self, id: u32) {
        let activity = self.changes.find::<Activity>();
        if activity.is_none() && !self.changes.logs[STATIC].recording {
            if self.self_and_descendants(id).any(|e| self.is_static(e)) {
                self.stamp_static(id); // unrecorded: only the revision moves
            }
            return;
        }
        let below: Vec<u32> = self.self_and_descendants(id).collect();
        for e in below {
            if let Some(at) = activity {
                self.stamp_at(at, e);
            }
            if self.is_static(e) {
                self.stamp_static(e);
            }
        }
    }

    /// `id`'s world pose and every descendant's may have changed ([`WorldPose`]).
    /// Skipped when `id` is still unread in the log: every stamp on this channel is
    /// a subtree walk, so its subtree was stamped with it (an entity joining it
    /// since was moved, and walked, itself).
    pub(in crate::ecs) fn stamp_pose_below(&mut self, id: u32) {
        let Some(at) = self.changes.find::<WorldPose>() else {
            return;
        };
        if self.changes.logs[at].pending(id) {
            return;
        }
        let below: Vec<u32> = self.self_and_descendants(id).collect();
        below.into_iter().for_each(|e| self.stamp_at(at, e));
    }
}

//! crates/ecs/src/changes/stamp.rs — where the World stamps its logs (#1705).
//!
//! A column write is [`World::touch`] (or [`World::touch_entity`] for a whole
//! entity): one call that notes the write for the edit journal and stamps the
//! log, so a raw write cannot do one and forget the other. The raw hecs writes
//! live in `funnel.rs`, `placement.rs` and `World::remove_rows` / `clear`, each
//! with the `ecs-write` lint's inline allow; a column write calls `touch` only
//! once the entity is found to hold the column.
//!
//! A derived channel is stamped at the source of its fact, its subtree included:
//! [`World::stamp_pose_below`] from `World::moved`, [`World::stamp_hierarchy_below`]
//! wherever an `active` flag or a parent link changes. A walk is taken only while a
//! cursor is open on the channel it feeds; with none, an `any` and one stamp keep
//! [`World::static_revision`] moving.

use super::{Activity, Channel, Children, Layer, WorldPose, STATIC};
use crate::World;

/// A subtree stamp's walk state, kept on the change log between walks so a warm
/// walk allocates nothing (`world_space::bounds`' flush walks the same way). A
/// walk started inside another's visit finds it taken and starts cold: correct,
/// only not allocation-free.
#[derive(Debug, Default)]
pub(super) struct Scratch {
    stack: Vec<u32>,
    /// The walk that last reached each id, so a walk yields it once.
    seen: Vec<u32>,
    walk: u32,
}

impl Scratch {
    /// Start a walk: a new stamp, which no id carries yet.
    fn begin(&mut self) {
        self.walk = self.walk.wrapping_add(1);
        if self.walk == 0 {
            self.seen.fill(0);
            self.walk = 1;
        }
    }

    /// Whether `id` is first reached in this walk (and mark it reached).
    fn first_visit(&mut self, id: u32) -> bool {
        let at = id as usize;
        if at >= self.seen.len() {
            self.seen.resize(at + 1, 0);
        }
        std::mem::replace(&mut self.seen[at], self.walk) != self.walk
    }
}

impl World {
    /// A write to column `T` of `id`: noted for the journal and stamped on `T`.
    pub(crate) fn touch<T: Channel>(&mut self, id: u32) {
        const { assert!(T::COLUMN, "a derived channel is stamped at its source") };
        self.note(id);
        if self.changes.get_mut().logs.len() == 1 {
            return; // only `StaticSet` exists: no column was ever opened
        }
        if let Some(at) = self.changes.get_mut().find::<T>() {
            self.stamp_at(at, id);
        }
    }

    /// `id` written whole (placed, overwritten or removed; `is_static` its flag):
    /// noted for the journal and stamped on every column, [`Activity`], [`Layer`],
    /// [`Children`] and, when static, [`super::StaticSet`]. [`WorldPose`] follows from
    /// `World::moved` (or, for a removal, its own walk), so its stamps are always a
    /// whole subtree's.
    pub(crate) fn touch_entity(&mut self, id: u32, is_static: bool) {
        self.note(id);
        if is_static {
            self.stamp_static(id);
        }
        if self.changes.get_mut().logs.len() == 1 {
            return;
        }
        let live = self.order.len();
        for log in self.changes.get_mut().logs.iter_mut().filter(|l| l.column) {
            log.stamp(id, live);
        }
        for at in [
            self.changes.get_mut().find::<Activity>(),
            self.changes.get_mut().find::<Layer>(),
            self.changes.get_mut().find::<Children>(),
        ] {
            at.into_iter().for_each(|at| self.stamp_at(at, id));
        }
    }

    fn stamp_at(&mut self, at: usize, id: u32) {
        let live = self.order.len();
        self.changes.get_mut().logs[at].stamp(id, live);
    }

    /// `id`'s membership of the static set may have changed ([`super::StaticSet`]).
    pub(crate) fn stamp_static(&mut self, id: u32) {
        self.stamp_at(STATIC, id);
    }

    /// `id`'s layer changed ([`Layer`]).
    pub(crate) fn stamp_layer(&mut self, id: u32) {
        if let Some(at) = self.changes.get_mut().find::<Layer>() {
            self.stamp_at(at, id);
        }
    }

    /// `id`'s child list changed ([`Children`]).
    pub(crate) fn stamp_children(&mut self, id: u32) {
        if let Some(at) = self.changes.get_mut().find::<Children>() {
            self.stamp_at(at, id);
        }
    }

    /// Whether `id` and everything below it are active in the hierarchy may have
    /// changed (an activation or a re-parent there, a placement that brings
    /// children): stamp the subtree on [`Activity`], and its static entities on
    /// [`super::StaticSet`].
    pub(crate) fn stamp_hierarchy_below(&mut self, id: u32) {
        let activity = self.changes.get_mut().find::<Activity>();
        if activity.is_none() && !self.changes.get_mut().logs[STATIC].recording {
            if self.self_and_descendants(id).any(|e| self.is_static(e)) {
                self.stamp_static(id); // unrecorded: only the revision moves
            }
            return;
        }
        self.each_below(id, |w, e| {
            if let Some(at) = activity {
                w.stamp_at(at, e);
            }
            if w.is_static(e) {
                w.stamp_static(e);
            }
        });
    }

    /// `id`'s world pose and every descendant's may have changed ([`WorldPose`]).
    /// Skipped when `id` is still unread in the log: every stamp on this channel is
    /// a subtree walk, so its subtree was stamped with it (an entity joining it
    /// since was moved, and walked, itself).
    pub(crate) fn stamp_pose_below(&mut self, id: u32) {
        let Some(at) = self.changes.get_mut().find::<WorldPose>() else {
            return;
        };
        if self.changes.get_mut().logs[at].pending(id) {
            return;
        }
        self.each_below(id, |w, e| w.stamp_at(at, e));
    }

    /// `visit` each id [`World::self_and_descendants`] yields, in its order, with the
    /// World writable: the walk keeps its stack and seen set on the change log
    /// instead of collecting the subtree first.
    fn each_below(&mut self, id: u32, mut visit: impl FnMut(&mut World, u32)) {
        let mut s = std::mem::take(&mut self.changes.get_mut().scratch);
        s.begin();
        s.stack.push(id);
        while let Some(e) = s.stack.pop() {
            if !self.contains(e) || !s.first_visit(e) {
                continue;
            }
            visit(self, e);
            let at = s.stack.len();
            self.push_children(e, &mut s.stack);
            s.stack[at..].reverse();
        }
        self.changes.get_mut().scratch = s;
    }
}

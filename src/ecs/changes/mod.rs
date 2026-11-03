//! src/ecs/changes/ — change detection: which entities changed on a channel since a
//! consumer last looked (#1705). Bevy's `Changed<T>` / `RemovedComponents<T>` and
//! Unity's `transform.hasChanged`, as one log per channel read through cursors.
//!
//! **Channels.** Every component column is one (`T` is the column's type: a
//! `ColliderComponent`, the `TransformComponent`, the `Core` flags). It is stamped
//! at the World's write funnels: a column borrowed mutably (`component_mut`,
//! `with_pair_mut`), attached, detached or taken (`set_component`,
//! `take_component`), and every column of an entity placed, overwritten or
//! removed. A mutable borrow counts as a change, as in Bevy: a write that changed
//! nothing is over-reported, which is safe; a missed one would be the bug. A
//! *derived* channel is a marker type stamped by hand where its fact can change;
//! [`StaticSet`] is the first.
//!
//! **Cursors.** A consumer opens a [`Cursor`] once ([`World::change_cursor`]) and
//! asks [`World::changes`] each time it runs: [`Changes::Ids`], the ids stamped
//! since its last read, ascending, each once; or [`Changes::Rebuild`] when the
//! cursor cannot be answered from the log (the World was cleared or replaced, a
//! Play snapshot restored, an Undo restored an entity, or the log dropped what the
//! cursor had not read yet). What an id holds *now* is the World's answer: a
//! consumer re-reads it, and an id that is gone or lost the column was removed.
//! Any number of cursors read one channel, each at its own pace.
//!
//! **Cost.** A channel no cursor was ever opened on costs a write one test and,
//! when another channel is being read, one lookup; nothing accumulates for it. A
//! read channel's log is bounded (`log.rs`).
//!
//! **Determinism.** The log is sim state: ids come back ascending, channels are
//! found by type and walked only in the order they were first opened, never by
//! `TypeId`. A consumer computes, for the ids it is given, exactly what its full
//! pass computes for them.

mod log;
#[cfg(test)]
mod static_tests;
#[cfg(test)]
mod tests;

use std::any::TypeId;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

use log::Log;

use super::world::World;

/// The derived channel stamped wherever the set of static entities active in the
/// hierarchy may have changed, with the entities whose membership may have moved:
/// a flag flipped, a static entity (or an ancestor of one) activated, deactivated
/// or re-parented, a static entity placed or removed. Its revision is
/// [`World::static_revision`].
pub struct StaticSet;

/// A consumer's place in one channel's log. Opened by [`World::change_cursor`].
pub struct Cursor<T> {
    epoch: u64,
    seen: u64,
    _channel: PhantomData<fn() -> T>,
}

/// What changed on a channel since a cursor's last read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Changes {
    /// The log cannot say: rebuild from the whole World.
    Rebuild,
    /// The ids stamped since the last read, ascending, each once.
    Ids(Vec<u32>),
}

/// Every World (and every `clear` or restore of one) takes a fresh epoch, so a
/// cursor never reads a log it was not opened on. Only compared, never read as sim.
static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);

fn fresh_epoch() -> u64 {
    NEXT_EPOCH.fetch_add(1, Ordering::Relaxed)
}

/// `StaticSet`'s index in `ChangeLogs::logs`: it is opened first, by every World.
const STATIC: usize = 0;

/// The World's logs.
#[derive(Debug)]
pub(in crate::ecs) struct ChangeLogs {
    epoch: u64,
    /// The logs, in the order their channels were first opened.
    logs: Vec<Log>,
    /// Channel -> index into `logs`. Looked up, never iterated.
    index: BTreeMap<TypeId, usize>,
    /// Debug names, parallel to `logs`.
    names: Vec<&'static str>,
}

impl Default for ChangeLogs {
    fn default() -> Self {
        let mut logs = Self {
            epoch: fresh_epoch(),
            logs: Vec::new(),
            index: BTreeMap::new(),
            names: Vec::new(),
        };
        logs.slot::<StaticSet>(false); // index `STATIC`; its revision is `static_revision`, read unopened
        logs
    }
}

impl ChangeLogs {
    fn slot<T: 'static>(&mut self, column: bool) -> &mut Log {
        let next = self.logs.len();
        let at = *self.index.entry(TypeId::of::<T>()).or_insert(next);
        if at == next {
            self.logs.push(Log::new(column));
            self.names.push(std::any::type_name::<T>());
        }
        &mut self.logs[at]
    }

    fn get<T: 'static>(&self) -> Option<&Log> {
        self.index.get(&TypeId::of::<T>()).map(|&at| &self.logs[at])
    }

    /// The World was replaced or rewound: every cursor reads `Rebuild` next.
    pub(in crate::ecs) fn new_epoch(&mut self) {
        self.epoch = fresh_epoch();
        self.logs.iter_mut().for_each(Log::reset);
    }
}

impl World {
    /// `id` changed on column `T`.
    pub(in crate::ecs) fn stamp<T: 'static>(&mut self, id: u32) {
        if self.changes.logs.len() == 1 {
            return; // only `StaticSet` exists: no column was ever opened
        }
        let live = self.order.len();
        let changes = &mut self.changes;
        if let Some(&at) = changes.index.get(&TypeId::of::<T>()) {
            changes.logs[at].stamp(id, live);
        }
    }

    /// `id` changed on every column at once (placed, overwritten, removed).
    pub(in crate::ecs) fn stamp_columns(&mut self, id: u32) {
        if self.changes.logs.len() == 1 {
            return;
        }
        let live = self.order.len();
        for log in self.changes.logs.iter_mut().filter(|l| l.column) {
            log.stamp(id, live);
        }
    }

    /// `id`'s membership of the static set may have changed ([`StaticSet`]).
    pub(in crate::ecs) fn stamp_static(&mut self, id: u32) {
        let live = self.order.len();
        self.changes.logs[STATIC].stamp(id, live);
    }

    /// Stamp every static entity in `id`'s subtree: an activation or a re-parent
    /// there changes whether they are active in the hierarchy.
    pub(in crate::ecs) fn stamp_static_below(&mut self, id: u32) {
        let statics: Vec<u32> = self
            .self_and_descendants(id)
            .filter(|&e| self.is_static(e))
            .collect();
        statics.into_iter().for_each(|e| self.stamp_static(e));
    }

    /// Open a cursor on channel `T`: it has read everything up to now.
    pub fn change_cursor<T: 'static>(&mut self) -> Cursor<T> {
        let column = TypeId::of::<T>() != TypeId::of::<StaticSet>();
        let seen = self.changes.slot::<T>(column).open();
        Cursor {
            epoch: self.changes.epoch,
            seen,
            _channel: PhantomData,
        }
    }

    /// What changed on `T` since `cursor`'s last read; the cursor then has read it all.
    pub fn changes<T: 'static>(&mut self, cursor: &mut Cursor<T>) -> Changes {
        let epoch = self.changes.epoch;
        let log = self.changes.slot::<T>(true);
        let ids = (cursor.epoch == epoch)
            .then(|| log.since(cursor.seen))
            .flatten();
        *cursor = Cursor {
            epoch,
            seen: log.open(),
            _channel: PhantomData,
        };
        ids.map_or(Changes::Rebuild, Changes::Ids)
    }

    /// Channel `T`'s revision: it moves on every stamp, opened or not (`0` for a
    /// channel never opened, other than [`StaticSet`]).
    pub fn change_revision<T: 'static>(&self) -> u64 {
        self.changes.get::<T>().map_or(0, |l| l.revision)
    }

    /// Each opened channel's name and the entries its log holds, in opening order:
    /// how much churn a game makes, per channel.
    pub fn change_stats(&self) -> Vec<(&'static str, usize)> {
        let sizes = self.changes.logs.iter().map(Log::len);
        self.changes.names.iter().copied().zip(sizes).collect()
    }
}

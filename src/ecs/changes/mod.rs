//! src/ecs/changes/ — change detection: which entities changed on a channel since a
//! consumer last looked (#1705). Bevy's `Changed<T>` / `RemovedComponents<T>` and
//! Unity's `transform.hasChanged`, as one log per channel read through cursors.
//!
//! **Channels** ([`Channel`], sealed). Every component column is one (`T` is the
//! column's type: a `ColliderComponent`, the `TransformComponent`,
//! `Vec<ScriptComponent>`, the `Core` flags). It is stamped at the World's write
//! funnels, in the same call that notes the write for the edit journal
//! (`stamp.rs`): a column borrowed mutably (`component_mut`, `with_mut_and_ref`),
//! attached, detached or taken (`set_component`, `take_component`), and every
//! column of an entity placed, overwritten or removed. A write to a column the
//! entity lacks stamps nothing (`ecs/funnel.rs`). A mutable borrow counts as
//! a change, as in Bevy: a write that changed nothing is over-reported, which is
//! safe; a missed one would be the bug. Two writes compare first because they are
//! hot: a Transform landing its old value and a collider's derived world box stamp
//! nothing. A *derived* channel is a marker stamped at the source of its fact, its
//! descendants included, so a consumer never expands ids itself: [`WorldPose`],
//! [`Activity`], [`Layer`] and [`StaticSet`].
//!
//! **Cursors.** A consumer opens a [`Cursor`] once ([`World::change_cursor`]) and
//! asks [`World::changes`] each time it runs: [`Changes::Ids`], the ids stamped
//! since its last read, ascending, each once; or [`Changes::Rebuild`] when the
//! cursor cannot be answered from the log (the World was cleared or replaced, a
//! Play snapshot restored, an Undo or Redo stepped ([`World::rewind_changes`]), or
//! the log dropped what the cursor had not read yet). What an id holds *now* is the World's answer: a
//! consumer re-reads it, and an id that is gone or lost the column was removed.
//! Any number of cursors read one channel, each at its own pace.
//!
//! **Cost.** A channel no cursor was ever opened on costs a write one test and,
//! when another channel is being read, one lookup; nothing accumulates for it, and
//! a derived channel's subtree walk is not taken. A read channel's log is bounded
//! (`log.rs`).
//!
//! **Determinism.** The log is sim state: ids come back ascending, channels are
//! found by type and walked only in the order they were first opened, never by
//! `TypeId`. A consumer computes, for the ids it is given, exactly what its full
//! pass computes for them.

mod channel;
#[cfg(test)]
mod derived_tests;
#[cfg(test)]
mod epoch_tests;
#[cfg(test)]
mod funnel_tests;
mod log;
#[cfg(test)]
pub(crate) mod ops_tests;
#[cfg(test)]
mod play_tests;
#[cfg(test)]
mod seam_tests;
mod stamp;
#[cfg(test)]
mod tests;

use std::any::TypeId;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

pub use channel::{Activity, Channel, Layer, StaticSet, WorldPose};
use log::Log;

use super::world::World;

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
    /// [`Channel::NAME`]s, parallel to `logs`.
    names: Vec<&'static str>,
    /// Epochs taken since this World was made: what a test counts an Undo by.
    rewinds: u64,
}

/// One opened channel's counters ([`World::change_stats`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChangeStat {
    pub name: &'static str,
    /// Entries the log holds now (bounded, `log.rs`).
    pub held: usize,
    /// Entries ever appended; per tick, with a reader every tick, the distinct ids
    /// dirtied in it.
    pub logged: u64,
}

impl Default for ChangeLogs {
    fn default() -> Self {
        let mut logs = Self {
            epoch: fresh_epoch(),
            logs: Vec::new(),
            index: BTreeMap::new(),
            names: Vec::new(),
            rewinds: 0,
        };
        logs.slot::<StaticSet>(); // index `STATIC`; its revision is `static_revision`, read unopened
        logs
    }
}

impl ChangeLogs {
    fn slot<T: Channel>(&mut self) -> &mut Log {
        let next = self.logs.len();
        let at = *self.index.entry(TypeId::of::<T>()).or_insert(next);
        if at == next {
            self.logs.push(Log::new(T::COLUMN));
            self.names.push(T::NAME);
        }
        &mut self.logs[at]
    }

    /// `T`'s index in `logs`: it exists once a cursor was opened on `T` (always, for
    /// [`StaticSet`]).
    fn find<T: Channel>(&self) -> Option<usize> {
        self.index.get(&TypeId::of::<T>()).copied()
    }

    /// The World was replaced or rewound: every cursor reads `Rebuild` next.
    pub(in crate::ecs) fn new_epoch(&mut self) {
        self.epoch = fresh_epoch();
        self.rewinds += 1;
        self.logs.iter_mut().for_each(Log::reset);
    }
}

impl World {
    /// The World was taken back to an earlier state as a whole (an Undo or Redo
    /// step, `scene::undo`): every cursor reads [`Changes::Rebuild`] next. One per
    /// step, not per entity: each costs every recording log a reset.
    pub fn rewind_changes(&mut self) {
        self.changes.new_epoch();
    }

    /// How many times the change logs were reset ([`World::rewind_changes`], a clear).
    #[cfg(test)]
    pub(crate) fn change_rewinds(&self) -> u64 {
        self.changes.rewinds
    }

    /// Open a cursor on channel `T`: it has read everything up to now.
    pub fn change_cursor<T: Channel>(&mut self) -> Cursor<T> {
        let seen = self.changes.slot::<T>().open();
        Cursor {
            epoch: self.changes.epoch,
            seen,
            _channel: PhantomData,
        }
    }

    /// What changed on `T` since `cursor`'s last read; the cursor then has read it all.
    pub fn changes<T: Channel>(&mut self, cursor: &mut Cursor<T>) -> Changes {
        let epoch = self.changes.epoch;
        let log = self.changes.slot::<T>();
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

    /// [`StaticSet`]'s revision, which moves on every stamp: `World::static_revision`.
    pub(in crate::ecs) fn static_set_revision(&self) -> u64 {
        self.changes.logs[STATIC].revision
    }

    /// Each opened channel's counters, in opening order: how much churn a game makes,
    /// per channel (`Debug.Stats`'s `changes_<name>`). [`StaticSet`] is always listed.
    pub fn change_stats(&self) -> Vec<ChangeStat> {
        let logs = self.changes.logs.iter();
        let names = self.changes.names.iter().copied();
        names
            .zip(logs)
            .map(|(name, log)| ChangeStat {
                name,
                held: log.len(),
                logged: log.logged,
            })
            .collect()
    }
}

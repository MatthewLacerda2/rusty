//! Change detection, as Bevy's `Changed<T>`: one log per channel, read through cursors. A mutable
//! borrow counts as a change (over-reporting is safe; a missed write is the bug). A read answers
//! the ids stamped since, ascending, or `Rebuild` after a clear, restore, Undo step or a dropped
//! tail.

#[cfg(test)]
mod carried_tests;
mod channel;
#[cfg(test)]
mod derived_case_tests;
#[cfg(test)]
mod derived_tests;
#[cfg(test)]
mod epoch_tests;
#[cfg(test)]
mod funnel_tests;
mod geometry;
mod log;
#[doc(hidden)]
pub mod ops;
#[cfg(test)]
mod quiet_tests;
#[cfg(test)]
mod seam_tests;
#[cfg(test)]
mod slot_tests;
mod stamp;
#[cfg(test)]
mod stamp_tests;
#[cfg(test)]
mod tests;

use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

pub use channel::{Activity, Channel, Children, Layer, StaticSet, WorldPose};
pub use geometry::StaticGeometry;
use log::Log;

use super::world::World;

pub struct Cursor<T> {
    epoch: u64,
    seen: u64,
    _channel: PhantomData<fn() -> T>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Changes {
    Rebuild,
    Ids(Vec<u32>),
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        matches!(self, Changes::Ids(ids) if ids.is_empty())
    }
}

// Only compared, never read as sim state: a cursor never reads a log it was not opened on.
static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);

fn fresh_epoch() -> u64 {
    NEXT_EPOCH.fetch_add(1, Ordering::Relaxed)
}

/// `StaticSet`'s index in `logs`: every World opens it first.
const STATIC: usize = 0;

const UNOPENED: u32 = u32::MAX;

fn carries<T: Channel>(row: &hecs::EntityRef<'_>) -> bool {
    row.has::<T>()
}

#[derive(Debug)]
pub(crate) struct ChangeLogs {
    epoch: u64,
    logs: Vec<Log>,
    index: Vec<u32>,
    names: Vec<&'static str>,
    rewinds: u64,
    scratch: stamp::Scratch,
    static_geometry: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChangeStat {
    pub name: &'static str,
    pub held: usize,
    pub logged: u64,
}

impl Default for ChangeLogs {
    fn default() -> Self {
        let mut logs = Self {
            epoch: fresh_epoch(),
            logs: Vec::new(),
            index: Vec::new(),
            names: Vec::new(),
            rewinds: 0,
            scratch: Default::default(),
            static_geometry: 0,
        };
        logs.slot::<StaticSet>();
        logs
    }
}

impl ChangeLogs {
    fn slot<T: Channel>(&mut self) -> &mut Log {
        let at = match self.find::<T>() {
            Some(at) => at,
            None => {
                let at = self.logs.len();
                if self.index.len() <= T::SLOT {
                    self.index.resize(T::SLOT + 1, UNOPENED);
                }
                self.index[T::SLOT] = at as u32;
                self.logs.push(Log::new(T::COLUMN.then_some(carries::<T>)));
                self.names.push(T::NAME);
                at
            }
        };
        &mut self.logs[at]
    }

    fn find<T: Channel>(&self) -> Option<usize> {
        let at = *self.index.get(T::SLOT)?;
        (at != UNOPENED).then_some(at as usize)
    }

    pub(crate) fn new_epoch(&mut self) {
        self.epoch = fresh_epoch();
        self.rewinds += 1;
        self.static_geometry += 1;
        self.logs.iter_mut().for_each(Log::reset);
    }

    pub(crate) fn reset_scratch(&mut self) {
        self.scratch = Default::default();
    }
}

impl World {
    pub fn rewind_changes(&mut self) {
        self.changes.get_mut().new_epoch();
    }

    /// A test hook: `scene::undo`'s tests count the rewinds a step costs.
    #[doc(hidden)]
    pub fn change_rewinds(&self) -> u64 {
        self.changes.borrow().rewinds
    }

    pub fn change_cursor<T: Channel>(&self) -> Cursor<T> {
        let mut changes = self.changes.borrow_mut();
        let seen = changes.slot::<T>().open();
        Cursor {
            epoch: changes.epoch,
            seen,
            _channel: PhantomData,
        }
    }

    /// One comparison; `false` does not promise an id the reader cares about.
    pub fn quiet<T: Channel>(&self, cursor: &Cursor<T>) -> bool {
        let changes = self.changes.borrow();
        let revision = changes.find::<T>().map(|at| changes.logs[at].revision);
        cursor.epoch == changes.epoch && revision == Some(cursor.seen)
    }

    pub fn changes<T: Channel>(&self, cursor: &mut Cursor<T>) -> Changes {
        self.read(cursor, Log::since)
            .map_or(Changes::Rebuild, Changes::Ids)
    }

    fn read<T: Channel, R>(
        &self,
        cursor: &mut Cursor<T>,
        read: impl FnOnce(&mut Log, u64) -> Option<R>,
    ) -> Option<R> {
        let mut changes = self.changes.borrow_mut();
        let epoch = changes.epoch;
        let log = changes.slot::<T>();
        let got = (cursor.epoch == epoch)
            .then(|| read(log, cursor.seen))
            .flatten();
        *cursor = Cursor {
            epoch,
            seen: log.open(),
            _channel: PhantomData,
        };
        got
    }

    pub(crate) fn static_set_revision(&self) -> u64 {
        self.changes.borrow().logs[STATIC].revision
    }

    pub fn change_stats(&self) -> Vec<ChangeStat> {
        let changes = self.changes.borrow();
        let logs = changes.logs.iter();
        let names = changes.names.iter().copied();
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

//! crates/ecs/src/walks.rs — how many times each column was walked, and how many
//! single-entity lookups each made, on this thread: what lets a test hold that a
//! frame walks no column it should not (#1706) and that a narrow query costs no
//! lookup per carrier. Walks are counted by every query built on
//! `World::each_core_with` or `World::ids_with_component_where` (`ids_with_*`,
//! `World::moving_bodies`); lookups by `World::component`, under every typed
//! getter. A test hook: compiled in every build, since the test that reads it is
//! another crate's, and counting only on a thread that asked [`of`] or
//! [`lookups_of`] once. A walk happens a few times a frame, so a build that never
//! asks pays one thread-local read per walk. A lookup is the hottest call the sim
//! makes, so lookups are counted only where `debug_assertions` holds (every test
//! profile): a release or ship build compiles the call away, and [`lookups_of`]
//! panics there rather than answer a silent 0. Re-exported as `world::walks`.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static WALKS: RefCell<BTreeMap<&'static str, u64>> = RefCell::default();
    static LOOKUPS: RefCell<BTreeMap<&'static str, u64>> = RefCell::default();
}

pub(crate) fn count(column: &'static str) {
    if ARMED.with(Cell::get) {
        WALKS.with(|w| *w.borrow_mut().entry(column).or_default() += 1);
    }
}

/// Counts nothing, and compiles to nothing, without `debug_assertions`.
#[inline(always)]
pub(crate) fn lookup(column: &'static str) {
    if cfg!(debug_assertions) && ARMED.with(Cell::get) {
        LOOKUPS.with(|l| *l.borrow_mut().entry(column).or_default() += 1);
    }
}

/// Walks of `T`'s column on this thread since the first call of this function
/// (or [`lookups_of`]) on it, which starts the count.
pub fn of<T>() -> u64 {
    read::<T>(&WALKS)
}

/// One-entity lookups of `T`'s column (`World::component`, under `World::mesh`
/// and every typed getter) on this thread since the first call of this function
/// (or [`of`]) on it, which starts the count.
///
/// # Panics
/// In a build without `debug_assertions` (release, ship), where lookups are not
/// counted: run the test in a debug or test profile.
pub fn lookups_of<T>() -> u64 {
    if cfg!(debug_assertions) {
        read::<T>(&LOOKUPS)
    } else {
        panic!(
            "walks::lookups_of: lookups are counted only with debug_assertions on; \
             run this test in a debug or test profile, not release"
        )
    }
}

fn read<T>(map: &'static std::thread::LocalKey<RefCell<BTreeMap<&'static str, u64>>>) -> u64 {
    ARMED.with(|a| a.set(true));
    let column = std::any::type_name::<T>();
    map.with(|m| m.borrow().get(column).copied().unwrap_or(0))
}

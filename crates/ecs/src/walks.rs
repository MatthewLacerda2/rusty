use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static WALKS: RefCell<BTreeMap<&'static str, u64>> = RefCell::default();
    static LOOKUPS: RefCell<BTreeMap<&'static str, u64>> = RefCell::default();
    static MOVES: Cell<u64> = const { Cell::new(0) };
    static COLLECTED: Cell<u64> = const { Cell::new(0) };
}

#[inline(always)]
pub(crate) fn collected(ids: usize) {
    if cfg!(debug_assertions) && ARMED.with(Cell::get) {
        COLLECTED.with(|c| c.set(c.get() + ids as u64));
    }
}

pub fn ids_collected() -> u64 {
    if !cfg!(debug_assertions) {
        panic!(
            "walks::ids_collected: ids are counted only with debug_assertions on; \
             run this test in a debug or test profile, not release"
        )
    }
    ARMED.with(|a| a.set(true));
    COLLECTED.with(Cell::get)
}

#[inline]
pub(crate) fn moved_if(moving: impl FnOnce() -> bool) {
    if ARMED.with(Cell::get) && moving() {
        MOVES.with(|m| m.set(m.get() + 1));
    }
}

pub fn moves() -> u64 {
    ARMED.with(|a| a.set(true));
    MOVES.with(Cell::get)
}

pub(crate) fn count(column: &'static str) {
    if ARMED.with(Cell::get) {
        WALKS.with(|w| *w.borrow_mut().entry(column).or_default() += 1);
    }
}

#[inline(always)]
pub(crate) fn lookup(column: &'static str) {
    if cfg!(debug_assertions) && ARMED.with(Cell::get) {
        LOOKUPS.with(|l| *l.borrow_mut().entry(column).or_default() += 1);
    }
}

pub fn of<T>() -> u64 {
    read::<T>(&WALKS)
}

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

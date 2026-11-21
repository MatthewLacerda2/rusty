use std::cell::Cell;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static READS: Cell<usize> = const { Cell::new(0) };
}

pub(crate) fn read() {
    if ARMED.with(Cell::get) {
        READS.with(|n| n.set(n.get() + 1));
    }
}

pub fn reads() -> usize {
    ARMED.with(|a| a.set(true));
    READS.with(Cell::get)
}

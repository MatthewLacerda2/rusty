//! The one test every path-keyed cache runs (#1207): `rusty_core::fs::written`'s
//! contract, that a write is seen by the next load.

/// Which of two distinguishable contents a bake writes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bake {
    A,
    B,
}

/// Drive one long-lived cache through the authoring loop at a single `path`:
/// load before anything is baked (a miss), bake A → load is A, re-bake B → load
/// is B. `bake` writes through the engine's own writer, `load` goes through the
/// cache under test, and `is` tells what a load returned. Catches both halves of
/// the bug kind: a failed load cached for good, and a re-bake that keeps the old
/// contents.
pub fn follows_rebakes<T: std::fmt::Debug>(
    path: &str,
    mut bake: impl FnMut(&str, Bake),
    mut load: impl FnMut(&str) -> T,
    is: impl Fn(&T, Bake) -> bool,
) {
    std::fs::remove_file(path).ok();
    let missing = load(path);
    assert!(
        !is(&missing, Bake::A) && !is(&missing, Bake::B),
        "nothing baked yet, got {missing:?}"
    );
    bake(path, Bake::A);
    let first = load(path);
    assert!(is(&first, Bake::A), "a late bake must load, got {first:?}");
    bake(path, Bake::B);
    let second = load(path);
    assert!(is(&second, Bake::B), "a re-bake must load, got {second:?}");
}

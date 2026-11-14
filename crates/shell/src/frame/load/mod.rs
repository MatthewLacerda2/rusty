//! crates/shell/src/frame/load/ — a `Scene.LoadAsync` in a window (#2123): the worker that
//! prepares it while frames go on (`worker`), and its GPU warm-up, a slice per frame
//! (`warm`), before it reads ready.

pub mod warm;
pub mod worker;

#[cfg(test)]
#[path = "warm_tests.rs"]
mod warm_tests;

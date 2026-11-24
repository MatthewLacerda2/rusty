//! The bake queue: several scenes baked one after another, unattended, and picked up at
//! the first unfinished one after a crash or a reboot. The scene is the unit that
//! resumes: a cut bake writes nothing, so a scene cut under its bake is baked again.

pub mod cli;
pub mod file;
pub mod live;
pub mod owner;
pub mod run;
pub mod scene;
pub mod select;

pub use file::{read, Entry, Queue, Read, State, QUEUE_PATH};
pub use owner::{Owner, LOCK_PATH};
pub use run::{Outcome, Resumed, Runner};
pub use select::{resolve, Selection};

#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod tests;

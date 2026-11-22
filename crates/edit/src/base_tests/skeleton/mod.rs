//! `rusty_scene::skeleton`: bones as GameObjects on a model instantiated from a
//! file, through a save and load, a re-import, a prefab and Play.

mod bones;
mod destroy_bone;
pub(super) mod fixture;
mod missing_slot;
mod persist;
mod prefab;
mod rebind;
mod refs;
mod reimport;

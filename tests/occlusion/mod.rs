//! The occlusion bake (`lighting::occlusion`) across a save, a load and a
//! runtime level swap: the paths through `edit`, above `lighting`, whose own tests
//! hold the bake and the fingerprint.

mod carry;
mod fresh_load;
mod unsaved;

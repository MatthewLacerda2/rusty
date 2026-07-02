//! src/scripting/callbacks.rs — the script lifecycle-callback names, in one place.
//!
//! These are the MonoBehaviour-style functions the engine looks up on the table a
//! script returns. `lifecycle` dispatches them, `discovery` uses the list to
//! recognize a MonoBehaviour, and the doc-drift gate
//! (`tests/callback_doc_drift.rs`, #309) checks `docs/scripting-api.md` against
//! it — one list, so dispatch, discovery, and doc can never disagree about which
//! callbacks exist.

/// Called once per script when Play begins, before the first frame ticks.
pub const START: &str = "Start";
/// Called every frame of play while the owning entity is active.
pub const UPDATE: &str = "Update";
/// Called once per trigger-overlap pair involving the owning entity, each frame
/// the overlap persists.
pub const ON_TRIGGER: &str = "OnTrigger";

/// Every callback the engine dispatches — the ground truth the doc gate reads.
pub const LIFECYCLE_CALLBACKS: &[&str] = &[START, UPDATE, ON_TRIGGER];

//! src/scene/snapshot.rs — edit-mode snapshot / restore around Play
//!
//! "Save as-is in EDIT mode" guarantee: edit-mode is authoritative.
//!   - Entering Play -> snapshot the edit scene (clone); run Play on the clone.
//!   - Exiting Play  -> restore the snapshot, discarding all play-mode mutation
//!                      (script/physics moves, health changes, spawns/despawns).
//! So Save never persists played state. Mirrors Unity's play-mode behaviour.
//!
//! Implemented as a SceneData snapshot taken on the play-state transition, applied
//! back via scene::serialize::from_scene on Stop.
//!
//! Allowed deps: scene::serialize, ecs, app (PlayState).
//! Status: SCAFFOLD — structure only; not yet implemented.

//! src/app/mod.rs — App + run-loop orchestration
//!
//! Owns the World, Resources and Schedule; advances stages each frame. Replaces the hand-inlined loop in main.rs.
//!
//! Allowed deps: hecs, app::*, components, time. NOT render/editor/dev.
//! Status: SCAFFOLD — structure only; not yet implemented.

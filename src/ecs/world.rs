//! src/ecs/world.rs — World wrapper
//!
//! Wraps hecs::World: spawn/despawn, find_by_name, name<->entity map. Fixes
//! hardcoded-ID fragility. `spawn_object()` always attaches a Transform (the one
//! mandatory component); every other component is optional.
//!
//! Allowed deps: hecs.
//! Status: SCAFFOLD — structure only; not yet implemented.

//! src/api/mod.rs — Engine API facade
//!
//! The single stable API surface shared by Lua scripts, the console REPL and
//! bot-players. The runtime namespaces (`Health`, `Time`, `Camera`, writable
//! `Input`, `Physics.Raycast/Shoot`) plus the dev-only `Debug` are currently
//! registered from `scripting::bindings` (issue #5); these files document each
//! namespace until the `api/` tree formally owns the registration.
//!
//! Allowed deps: scripting, all components.
//! Status: PARTIAL — namespaces live; registration still hosted in `scripting/`.

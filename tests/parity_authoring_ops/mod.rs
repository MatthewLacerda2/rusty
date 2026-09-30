//! Editor↔Lua-API convergence over the shared `scene::authoring` ops (#287).
//!
//! Each first-class component whose card was routed through a shared op has its Lua
//! `*.Set*` bindings AND the shared op driven against two sibling entities; the
//! resulting component state must be byte-identical. That pins single-sourcing: the
//! editor card calls the same op the binding does, so the egui panel and the Lua
//! surface can never drift (mirrors `material_library::api_parity::lua_material_api_and_shared_op_converge`).

mod animator;
mod graphics;
mod light;
mod nav_agent;
mod particles;
mod physics;

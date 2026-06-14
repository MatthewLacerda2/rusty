//! src/api/health.rs — Health API
//!
//! `Health.Get/Set/Heal/Damage` over `components::health::HealthComponent`.
//!
//! Allowed deps: components::health.
//! Status: IMPLEMENTED — Lua bindings live in
//! `scripting::bindings::combat::register_health` (issue #5). This file documents
//! the surface; the registration is in `scripting/` until the `api/` tree owns it.

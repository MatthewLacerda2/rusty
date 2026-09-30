//! The integration suite: every file in `tests/` compiled into **one** binary (#483).
//!
//! Cargo's default builds each `tests/*.rs` as its own executable, and each of those
//! links the whole engine — wgpu/naga, egui, rapier3d, vendored Lua. At 54 files and
//! two feature sets that was 108 link steps per CI run, and most of the suite's time
//! and `target/` size. `Cargo.toml` sets `autotests = false` and roots one `[[test]]`
//! target here instead, so the engine is linked once per feature set.
//!
//! **Adding a test file means adding its `mod` line below** — Cargo no longer finds it
//! on its own. `layout` fails the suite, naming the file, if one is forgotten.
//!
//! Every module now shares one process, so a test must not rely on having a process to
//! itself: no fixed temp paths shared with another file, no `set_var`/`set_current_dir`.
//! Filter by module path: `cargo nextest run -E 'binary(integration)' physics_` runs the
//! physics files (`cargo test --test integration physics_` works too).

mod layout;

// ── Both feature sets ────────────────────────────────────────────────────────────
mod animation_runtime;
mod animator_graph_api;
mod animator_parameters;
mod asset_scene_reference;
mod audio_api;
mod callback_doc_drift;
mod decals_api;
mod default_ambient;
mod graphics_api;
mod input_api;
mod kinematic_gravity;
mod layers_api;
mod light_probes;
mod material_authoring;
mod material_library;
mod nav_agents_values;
mod navigation_heightfield;
mod navmesh_settings;
mod parity_authoring_ops;
mod particles_api;
mod particles_collision;
mod particles_determinism;
mod physics_character;
mod physics_contacts;
mod physics_gravity;
mod physics_queries;
mod physics_rapier;
mod prefab_api;
mod procgen_bake;
mod procgen_ops;
mod procgen_ops_math;
mod proptest_scene;
mod reflection_probes;
mod scene_roundtrip;
mod sound_api;
mod starter_materials;
mod texture_api;
mod transform_api;
mod ui_api;
mod ui_graphics_api;
mod video_api;

// ── Dev layer only (harness, session, MCP, `Debug.*`) ────────────────────────────
#[cfg(feature = "dev")]
mod api_doc_drift;
#[cfg(feature = "dev")]
mod application_quit;
#[cfg(feature = "dev")]
mod bot_player_session;
#[cfg(feature = "dev")]
mod frame_stats;
#[cfg(feature = "dev")]
mod harness_determinism;
#[cfg(feature = "dev")]
mod input_harness;
#[cfg(feature = "dev")]
mod lua_determinism;
#[cfg(feature = "dev")]
mod mcp_attach;
#[cfg(feature = "dev")]
mod mcp_bridge;
#[cfg(feature = "dev")]
mod nav_bounds_boot;
#[cfg(feature = "dev")]
mod pause_step;
#[cfg(feature = "dev")]
mod proptest_harness;
#[cfg(feature = "dev")]
mod structural_authoring;
#[cfg(feature = "dev")]
mod time_scale;

// ── Needs a GPU or software adapter (skips without one); all dev-only ────────────
// Kept under one `gpu::` path so a runner can tell them apart by name alone.
#[cfg(feature = "dev")]
mod gpu;

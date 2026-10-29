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
//! itself: scratch files go under `temp::dir()` (one per test), no `set_var`/`set_current_dir`.
//! Filter by module path: `cargo nextest run -E 'binary(integration)' physics_` runs the
//! physics files (`cargo test --test integration physics_` works too).

mod layout;
mod lua;
mod play;
mod temp;

// ── Both feature sets ────────────────────────────────────────────────────────────
mod animation_runtime;
mod animator;
mod api_doc_layout;
mod asset_scene_reference;
mod audio_api;
mod callback_doc_drift;
mod camera_stack_api;
mod camera_target_api;
mod cast_shadows;
mod character_controller;
mod conversion_gate;
mod decals;
mod default_ambient;
mod entity_arg_api;
mod gamepad_api;
mod graphics;
mod hitboxes;
mod ik;
mod input_api;
mod joint_api;
mod layers_api;
mod layers_from_lua;
mod light_probes;
mod lod_group_api;
mod material_authoring;
mod material_library;
mod nav;
mod navigation_layered;
mod navmesh_settings;
mod parity_authoring_ops;
mod particles;
mod physics;
mod plane_subdivisions;
mod prefab_api;
mod procgen;
mod proptest;
mod ragdoll;
mod reflection_probes;
mod ribbons;
mod scene_format;
mod scene_roundtrip;
mod selectable_api;
mod sound_api;
mod sound_song_api;
mod starter_materials;
mod static_flag_api;
mod texture_api;
mod transform_api;
mod ui;
mod video_api;

// ── Dev layer only (harness, session, MCP, `Debug.*`) ────────────────────────────
#[cfg(feature = "dev")]
mod api_doc_drift;
#[cfg(feature = "dev")]
mod application_quit;
#[cfg(feature = "dev")]
mod audio_capture;
#[cfg(feature = "dev")]
mod bot_player_session;
#[cfg(feature = "dev")]
mod busy_replay;
#[cfg(feature = "dev")]
mod doc_examples;
#[cfg(feature = "dev")]
mod editor_play;
#[cfg(feature = "dev")]
mod export;
#[cfg(feature = "dev")]
mod frame_stats;
#[cfg(feature = "dev")]
mod harness_determinism;
#[cfg(feature = "dev")]
mod harness_workspace;
#[cfg(feature = "dev")]
mod input_harness;
#[cfg(feature = "dev")]
mod lua_determinism;
#[cfg(feature = "dev")]
mod mcp_attach;
#[cfg(feature = "dev")]
mod mcp_bench_embed;
#[cfg(feature = "dev")]
mod mcp_bridge;
#[cfg(feature = "dev")]
mod pause_step;
#[cfg(feature = "dev")]
mod play_project;
#[cfg(feature = "dev")]
mod player_controller;
#[cfg(feature = "dev")]
mod provider_keys;
#[cfg(feature = "dev")]
mod reparent_bounds;
#[cfg(feature = "dev")]
mod scenario_api;
#[cfg(feature = "dev")]
mod script_timers;
#[cfg(feature = "dev")]
mod structural_authoring;
#[cfg(feature = "dev")]
mod time_scale;
#[cfg(feature = "dev")]
mod tweens;

// ── Needs a GPU or software adapter (skips without one); all dev-only ────────────
// Kept under one `gpu::` path so a runner can tell them apart by name alone.
#[cfg(feature = "dev")]
mod gpu;

//! The integration suite: every file in `tests/` compiled into one binary, so the engine is linked
//! once per feature set. **Adding a test file means adding its `mod` line below**; `layout` fails
//! the suite naming a forgotten file. Modules share one process: scratch files go under
//! `temp::dir()`, no `set_var`/`set_current_dir`.

mod layout;
mod lua;
mod play;
#[cfg(feature = "dev")]
mod spawn;
mod temp;

mod animation_runtime;
mod animator;
mod api_doc_layout;
mod asset_scene_reference;
mod audio_api;
mod callback_doc_drift;
mod camera;
mod cast_shadows;
mod character_controller;
mod conversion_gate;
mod damage;
mod decals;
mod default_ambient;
mod entity_arg_api;
mod floating_origin;
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
mod mesh_culling;
mod nav;
mod navigation_layered;
mod navmesh_settings;
mod occlusion;
mod parity_authoring_ops;
mod particles;
mod perception;
mod physics;
mod plane_subdivisions;
mod post_effect_cases;
mod post_effect_layers;
mod prefab_api;
mod process_order;
mod procgen;
mod proptest;
mod ragdoll;
mod reflection_probes;
mod ribbons;
mod runtime_lights;
mod scene_format;
mod scene_name_api;
mod scene_roundtrip;
mod selectable_api;
mod sound_api;
mod sound_song_api;
mod starter_materials;
mod static_flag_api;
mod texture_api;
mod texture_library;
mod transform_api;
mod ui;
mod video_api;

#[cfg(feature = "dev")]
mod api_doc_drift;
#[cfg(feature = "dev")]
mod application_quit;
#[cfg(feature = "dev")]
mod audio_capture;
#[cfg(feature = "dev")]
mod bin_help;
#[cfg(feature = "dev")]
mod bot_player_session;
#[cfg(feature = "dev")]
mod busy_replay;
#[cfg(feature = "dev")]
mod demo_replay;
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
mod project_open;
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

// Need a GPU or software adapter (skip without one); under one `gpu::` path so a runner tells them
// apart by name.
#[cfg(feature = "dev")]
mod gpu;

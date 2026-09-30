//! Integration tests that render: they build a headless [`Renderer`] (directly, or
//! through `screenshot::capture` / `Debug.Preview`) and skip when no adapter exists.
//!
//! Grouped under one `gpu::` module path so they can be identified by name: nextest
//! runs each test in its own process, where the in-process headless budget
//! (`render::setup::budget`) cannot bound them, so the `gpu` test group in
//! `.config/nextest.toml` selects them by this path instead. A new test that renders
//! belongs here — building a renderer from a test outside it panics in debug builds
//! (`render::setup::gpu_rule`).
//!
//! [`Renderer`]: rusty::render::Renderer

mod cascaded_shadows_screenshot;
mod custom_postfx_screenshot;
mod emissive_factor_screenshot;
mod fog_consistency_screenshot;
mod fog_modes_screenshot;
mod fog_scene;
mod frame_stats_render;
mod fxaa_screenshot;
mod instancing_budget;
mod material_maps_screenshot;
mod normal_emissive_maps_screenshot;
mod particle_modes_screenshot;
mod particle_scene;
mod particle_soft_lit_screenshot;
mod postfx_screenshot;
mod preview_api;
mod skinned_shadows_screenshot;
mod skinning_joint_cap;
mod ssao_screenshot;
mod transparent_sorting_screenshot;
mod ui_hud_scene;
mod ui_hud_screenshot;
mod ui_marker_screenshot;
mod ui_text_screenshot;
mod world_ui_fog_screenshot;
mod world_ui_scene;
mod world_ui_screenshot;

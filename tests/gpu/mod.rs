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

mod bloom_screenshot;
mod cascaded_shadows_screenshot;
mod custom_postfx_screenshot;
mod directional_lightmap_screenshot;
#[cfg(feature = "editor")]
mod editor_capture;
mod emissive_factor_screenshot;
mod fog;
mod frame_stats_render;
mod fxaa_screenshot;
mod instancing_budget;
mod lightmap_screenshot;
mod linear_data_maps_screenshot;
mod material_maps_screenshot;
mod metal_reflection_screenshot;
mod normal_emissive_maps_screenshot;
mod panorama_pole_screenshot;
mod panorama_sky_screenshot;
mod particle;
mod postfx;
mod preview_api;
mod preview_material;
mod probe_ambient_screenshot;
mod render_texture_scene;
mod render_texture_screenshot;
mod ribbons_screenshot;
mod skinned_shadows_screenshot;
mod skinning_joint_cap;
mod ssao_screenshot;
mod transparent_sorting_screenshot;
mod ui;
mod world_ui;

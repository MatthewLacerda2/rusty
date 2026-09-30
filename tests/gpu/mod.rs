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

mod emissive_factor_screenshot;
mod fxaa_screenshot;
mod material_maps_screenshot;
mod normal_emissive_maps_screenshot;
mod postfx_screenshot;
mod preview_api;
mod transparent_sorting_screenshot;

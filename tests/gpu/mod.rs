//! Integration tests that render: they build a headless [`Renderer`] (directly, or
//! through `screenshot::capture` / `Debug.Preview`) and skip when no adapter exists.
//!
//! Grouped under one `gpu::` module path so they can be identified by name — the
//! in-process headless budget (`render::setup::budget`) bounds how many renderers are
//! alive at once, and a runner that isolates tests per process needs its own bound.
//! A new test that renders belongs here, not beside the adapter-free tests.
//!
//! [`Renderer`]: rusty::render::Renderer

mod emissive_factor_screenshot;
mod fxaa_screenshot;
mod material_maps_screenshot;
mod normal_emissive_maps_screenshot;
mod postfx_screenshot;
mod preview_api;
mod transparent_sorting_screenshot;

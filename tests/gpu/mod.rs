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

use std::path::Path;

/// The mean of every RGB channel of the PNG at `path`, in `0..=255`.
pub fn mean_brightness(path: &Path) -> f64 {
    mean(&image::open(path).expect("open the capture").to_rgb8())
}

/// The mean of every RGB channel of `img`, in `0..=255`.
pub fn mean(img: &image::RgbImage) -> f64 {
    let sum: u64 = img.pixels().flat_map(|p| p.0).map(u64::from).sum();
    sum as f64 / (img.pixels().len() * 3) as f64
}

/// Give `scene` a black skybox panorama, so only its own lights and emissives light
/// it. `ambient_intensity = 0` removes the ambient term only: the procedural sky
/// still lights every surface, even a fully rough one, through its environment
/// reflection (#1543), as Unity's skybox does.
pub fn black_sky(scene: &mut rusty::scene::Scene) {
    let path = crate::temp::dir().join("black_sky.png");
    image::RgbaImage::from_pixel(2, 1, image::Rgba([0, 0, 0, 255]))
        .save(&path)
        .expect("write the black sky");
    scene.skybox_path = path.to_string_lossy().into_owned();
}

mod bloom_screenshot;
mod cascaded_shadows_screenshot;
mod custom_postfx_screenshot;
mod directional_lightmap_screenshot;
#[cfg(feature = "editor")]
mod editor_capture;
#[cfg(feature = "editor")]
mod editor_capture_demo;
mod emissive_factor_screenshot;
mod environment_brdf;
mod fog;
mod frame_stats_render;
mod fxaa_screenshot;
mod harness_size_screenshot;
mod instancing_budget;
mod lightmap_screenshot;
mod linear_data_maps_screenshot;
mod material_maps_screenshot;
mod metal_reflection_screenshot;
mod multi_material_screenshot;
mod normal_emissive_maps_screenshot;
mod normal_matrix_screenshot;
mod panorama_pole_screenshot;
mod panorama_sky_screenshot;
mod particle;
mod postfx;
mod preview_api;
mod preview_material;
mod probe_ambient_screenshot;
mod reflection_probe_per_object_screenshot;
mod render_texture_scene;
mod render_texture_screenshot;
mod ribbons_screenshot;
mod skinned_shadows_screenshot;
mod skinning_joint_cap;
mod sky_reflection_screenshot;
mod ssao_screenshot;
mod transparent_sorting_screenshot;
mod transparent_surfaces;
mod ui;
mod world_ui;

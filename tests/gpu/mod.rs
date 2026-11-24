//! Integration tests that render, skipping when no adapter exists. Grouped under one
//! `gpu::` path because nextest's `gpu` test group (`.config/nextest.toml`) selects
//! them by it; a test that builds a renderer outside it panics in debug builds
//! (`render::setup::gpu_rule`).

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
/// it: `ambient_intensity = 0` leaves the procedural sky lighting every surface.
pub fn black_sky(scene: &mut rusty::scene::Scene) {
    let path = crate::temp::dir().join("black_sky.png");
    image::RgbaImage::from_pixel(2, 1, image::Rgba([0, 0, 0, 255]))
        .save(&path)
        .expect("write the black sky");
    scene.skybox_path = path.to_string_lossy().into_owned();
}

mod bake_queue_cli;
mod baked_specular;
mod bloom_screenshot;
mod bounds_centre;
mod cascaded_shadows_screenshot;
mod custom_postfx_screenshot;
mod detail_layer;
mod detail_mask;
mod diffuse_model_screenshot;
mod directional_lightmap_screenshot;
#[cfg(feature = "editor")]
mod editor_capture;
mod emissive_factor_screenshot;
mod environment_brdf;
mod fallback_probe_screenshot;
mod fog;
mod frame_stats_render;
mod fxaa_screenshot;
mod gizmo_screenshot;
mod harness_size_screenshot;
mod hdr_overflow_screenshot;
mod height_map;
mod instancing_budget;
mod lightmap_formats_screenshot;
mod lightmap_screenshot;
mod linear_data_maps_screenshot;
mod material_maps_screenshot;
mod metal_reflection_screenshot;
mod multi_material_screenshot;
mod normal_emissive_maps_screenshot;
mod normal_matrix_screenshot;
mod occlusion_specular_maps_screenshot;
mod panorama_pole_screenshot;
mod panorama_sky_screenshot;
mod particle;
mod play_restore;
mod postfx;
mod preview_api;
mod preview_material;
mod probe_ambient_screenshot;
mod probe_normalisation_screenshot;
mod proxy_volume_clamp;
mod proxy_volume_screenshot;
mod reflection_probe_per_object_screenshot;
mod reflection_probe_seam_screenshot;
mod render_texture_scene;
mod render_texture_screenshot;
mod ribbons_screenshot;
mod runtime_light_shadow;
mod settle_frames;
mod shared_mesh_edit;
mod skinned_shadows_screenshot;
mod skinning_joint_cap;
mod sky_reflection_screenshot;
mod specular_aa_screenshot;
mod ssao_screenshot;
mod taa_cut_screenshot;
mod test_games;
mod toksvig;
mod transparent_sorting_screenshot;
mod transparent_surfaces;
mod ui;
mod warm;
mod world_ui;

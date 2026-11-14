//! A page list past the device's limits (#1704): refused with a warning, the scene
//! left unlit by lightmaps, instead of `create_texture` panicking on every load of
//! a file that holds too many pages.

use super::finish;
use super::load::Loaded;
use super::upload::{fits, MAX_ARRAY_BYTES};
use crate::test_gpu::headless_or_skip;
use rusty_scene::lighting::settings::MAX_PAGE;
use rusty_lighting::lightmap::MAX_PAGES;

#[test]
fn the_bake_s_page_limit_is_the_array_the_renderer_asks_for() {
    let limits = wgpu::Limits::default();
    assert_eq!(limits.max_texture_array_layers as usize, MAX_PAGES);
    assert!(limits.max_texture_dimension_2d >= MAX_PAGE);
}

#[test]
fn fits_refuses_more_layers_or_a_larger_edge_than_the_limits() {
    let small = wgpu::Limits {
        max_texture_array_layers: 4,
        max_texture_dimension_2d: 64,
        ..wgpu::Limits::default()
    };
    assert_eq!(fits(&small, 64, 4, 0), Ok(()));
    let layers = fits(&small, 64, 5, 0).unwrap_err();
    assert!(
        layers.contains("5 pages") && layers.contains("the 4 layers"),
        "{layers}"
    );
    let edge = fits(&small, 128, 1, 0).unwrap_err();
    assert!(edge.contains("128²") && edge.contains("64²"), "{edge}");
}

#[test]
fn fits_refuses_an_array_past_the_byte_budget() {
    let limits = wgpu::Limits::default();
    assert_eq!(fits(&limits, 1024, 8, MAX_ARRAY_BYTES), Ok(()));
    let err = fits(&limits, 1024, 8, MAX_ARRAY_BYTES + 1).unwrap_err();
    assert!(err.contains("1024 MiB") && err.contains("8 pages"), "{err}");
}

#[test]
fn gpu_too_many_pages_leave_the_scene_unlit_instead_of_panicking() {
    let Some(renderer) = headless_or_skip(4, 4) else {
        return;
    };
    let (device, queue) = (&renderer.device, &renderer.queue);
    let layers = device.limits().max_texture_array_layers as usize + 1;
    let loaded = Loaded {
        format: wgpu::TextureFormat::Rgba8Unorm,
        edge: 1,
        layers: vec![vec![vec![0; 4]]; layers],
    };
    let list = vec!["too_many.ktx2".to_string()];
    assert!(finish(device, queue, &list, Ok(loaded)).is_none());
}

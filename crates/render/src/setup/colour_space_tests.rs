use glam::Vec3;

use crate::{readback, RenderView, OFFSCREEN_FORMAT};
use rusty_components::{MaterialAsset, MaterialComponent, Tonemap};
use rusty_scene::{Camera, MeshComponent, PostProcessingComponent, Scene};

/// What the window surface typically picks (`surface_config` takes the first sRGB
/// format; BGRA is what Vulkan and Metal list first).
const WINDOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

/// sRGB-encoding linear 0.5 once gives 188/255; twice gives ~225, never gives 128.
const ONE_ENCODE_OF_HALF: u8 = 188;

fn mid_grey_scene() -> Scene {
    let mut scene = Scene::new();
    crate::test_gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let id = scene.add_entity("Grey".to_string());
    let (vertices, indices) = rusty_components::mesh::primitives::generate_box(8.0, 8.0, 8.0);
    scene
        .world
        .set_mesh(id, Some(MeshComponent::new("Box", vertices, indices)));
    scene
        .world
        .set_material(id, Some(MaterialComponent::new("grey".to_string())));
    scene.materials.insert(
        "grey".to_string(),
        MaterialAsset {
            base_color: [0.0; 3],
            emissive: [0.5; 3],
            ..MaterialAsset::default()
        },
    );
    let mut vc = rusty_scene::create::defaults::default_post_processing();
    vc.active = true;
    vc.bloom.active = false;
    vc.ssr.active = false;
    vc.tonemapping.mode = Tonemap::None;
    scene.world.set_post_processing(id, Some(vc));
    scene
        .world
        .set_camera(id, Some(rusty_components::CameraComponent::default()));
    scene
}

fn centre_pixel(
    renderer: &mut crate::Renderer,
    scene: &Scene,
    format: wgpu::TextureFormat,
) -> [u8; 3] {
    let mut view = RenderView::offscreen(&renderer.device, format, 16, 16);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 12.0), -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    let texture = view.color_target().unwrap();
    let px = readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, 16, 16);
    let i = (8 * 16 + 8) * 4;
    let p = [px[i], px[i + 1], px[i + 2]];
    match format {
        wgpu::TextureFormat::Bgra8UnormSrgb => [p[2], p[1], p[0]],
        _ => p,
    }
}

#[test]
fn gpu_mid_grey_encodes_once_and_identically_in_window_and_headless_formats() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let scene = mid_grey_scene();
    let headless = centre_pixel(&mut renderer, &scene, OFFSCREEN_FORMAT);
    let window = centre_pixel(&mut renderer, &scene, WINDOW_FORMAT);
    assert_eq!(
        headless, window,
        "window and screenshot disagree about colour"
    );
    for c in headless {
        assert!(
            c.abs_diff(ONE_ENCODE_OF_HALF) <= 2,
            "linear 0.5 should encode once to ~{ONE_ENCODE_OF_HALF}, got {headless:?}"
        );
    }
}

#[test]
fn every_final_target_format_is_srgb() {
    assert!(OFFSCREEN_FORMAT.is_srgb());
    assert!(WINDOW_FORMAT.is_srgb());
}

#[test]
fn a_new_volume_is_display_gamma_neutral() {
    let vc: PostProcessingComponent = rusty_scene::create::defaults::default_post_processing();
    assert_eq!(vc.tonemapping.display_gamma, 1.0);
}

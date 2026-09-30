//! One linear→display encode, owned by the target format (#415): a known linear
//! mid-grey must reach the same 8-bit value through the window's format and the
//! headless screenshot's, and that value must be *one* sRGB encode of it.

use glam::Vec3;

use crate::components::{MaterialAsset, MaterialComponent, Tonemap};
use crate::render::{readback, RenderView, OFFSCREEN_FORMAT};
use crate::scene::{Camera, DirtyFlag, MeshComponent, Scene, VisualCorrectionComponent};

/// What the window surface typically picks (`surface_config` takes the first sRGB
/// format; BGRA is what Vulkan and Metal list first).
const WINDOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

/// sRGB-encoding linear 0.5 once gives 188/255; twice gives ~225, never gives 128.
const ONE_ENCODE_OF_HALF: u8 = 188;

/// A box filling the view whose only light is a flat linear-0.5 emissive, graded
/// with every post-FX knob neutral (no tonemap), so the composite outputs 0.5.
fn mid_grey_scene() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_intensity = 0.0;
    let id = scene.add_entity("Grey".to_string());
    let (vertices, indices) = crate::components::mesh::primitives::generate_box(8.0, 8.0, 8.0);
    scene.world.set_mesh(
        id,
        Some(MeshComponent {
            primitive_type: "Box".to_string(),
            asset_ref: None,
            vertices,
            indices,
            bind_palette: Vec::new(),
            skin: None,
            clips: Vec::new(),
            pose_palette: Vec::new(),
            is_dirty: DirtyFlag::new(true),
        }),
    );
    scene.world.set_material(
        id,
        Some(MaterialComponent {
            material: "grey".to_string(),
        }),
    );
    scene.materials.insert(
        "grey".to_string(),
        MaterialAsset {
            base_color: [0.0; 3],
            emissive: [0.5; 3],
            ..MaterialAsset::default()
        },
    );
    let mut vc = crate::scene::authoring::defaults::default_visual_correction();
    vc.active = true;
    vc.bloom_active = false;
    vc.ssr_active = false;
    vc.tonemap = Tonemap::None;
    scene.world.set_visual_correction(id, Some(vc));
    scene
}

/// Render `scene` into a fresh `format` view and read back its centre pixel as RGB.
fn centre_pixel(
    renderer: &mut crate::render::Renderer,
    scene: &Scene,
    format: wgpu::TextureFormat,
) -> [u8; 3] {
    let mut view = RenderView::offscreen(&renderer.device, format, 16, 16, 2);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 12.0), -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, false, &[]);
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
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(16, 16) else {
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

/// The adapter-free half: both formats the final pass can write are sRGB, so neither
/// path can silently skip (or, with the shader, double) the encode.
#[test]
fn every_final_target_format_is_srgb() {
    assert!(OFFSCREEN_FORMAT.is_srgb());
    assert!(WINDOW_FORMAT.is_srgb());
}

/// A neutral volume (the defaults a new one is created with) leaves the image alone.
#[test]
fn a_new_volume_is_display_gamma_neutral() {
    let vc: VisualCorrectionComponent =
        crate::scene::authoring::defaults::default_visual_correction();
    assert_eq!(vc.gamma, 1.0);
}

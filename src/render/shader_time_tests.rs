//! Shaders see the sim's game time (#398): a baked `uv_scroll_stripes` surface
//! variant renders different pixels at two `Scene::shader_time` values, and the
//! same pixels when the time repeats — so it scrolls, and only with game time.

use glam::Vec3;

use crate::components::{MaterialAsset, MaterialComponent};
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{Camera, DirtyFlag, MeshComponent, Scene};
use crate::shadergen::assemble::assemble;
use crate::shadergen::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};

const RES: u32 = 32;

/// A surface variant with strong, slow-to-alias stripes: a quarter game second moves
/// them a quarter stripe, which changes every pixel's band brightness.
fn stripes_shader() -> std::path::PathBuf {
    let params = [("frequency", 2.0), ("strength", 1.0), ("speed", 1.0)]
        .into_iter()
        .map(|(k, v)| (k.to_string(), ParamValue::Scalar(v)))
        .collect();
    let recipe = ShaderRecipe {
        pass: PassKind::Surface,
        name: "scroll".into(),
        blocks: vec![BlockSel {
            id: "uv_scroll_stripes".into(),
            params,
        }],
    };
    let base = std::fs::read_to_string("assets/shaders/shader.wgsl").unwrap();
    let path = crate::test_temp::dir().join(format!("rusty_398_{}.wgsl", std::process::id()));
    std::fs::write(&path, assemble(&recipe, &base).unwrap()).unwrap();
    path
}

/// An emissive box filling the view, so the stripes are the only variation.
fn glowing_box() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let id = scene.add_entity("Glow".to_string());
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
            material: "glow".to_string(),
        }),
    );
    let glow = MaterialAsset {
        emissive: [0.6; 3],
        ..MaterialAsset::default()
    };
    scene.materials.insert("glow".to_string(), glow);
    scene
}

fn frame_at(renderer: &mut Renderer, scene: &mut Scene, shader: &str, time: f32) -> Vec<u8> {
    scene.shader_time = time;
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, RES, RES, 2);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 12.0), -90.0, 0.0);
    renderer.render_preview_with_shader(&mut view, scene, &cam, &out, shader);
    let texture = view.color_target().unwrap();
    readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, RES, RES)
}

#[test]
fn gpu_uv_scroll_stripes_scroll_with_game_time_only() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let path = stripes_shader();
    let shader = path.to_str().unwrap();
    let mut scene = glowing_box();
    let first = frame_at(&mut renderer, &mut scene, shader, 0.0);
    let again = frame_at(&mut renderer, &mut scene, shader, 0.0);
    let later = frame_at(&mut renderer, &mut scene, shader, 0.25);
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        first, again,
        "the same game time must render the same pixels"
    );
    let changed = first.iter().zip(&later).filter(|(a, b)| a != b).count();
    assert!(
        changed > first.len() / 4,
        "stripes did not scroll: only {changed} of {} bytes changed",
        first.len()
    );
}

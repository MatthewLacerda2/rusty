//! The gameplay surface blocks on the GPU (#401): each one, baked and named by a
//! material, renders a lit, non-black sphere that differs from the plain shader;
//! the animated ones change with game time and repeat when it does. The sphere's
//! base map is missing, so it shows the checker — which `uv_scroll` visibly moves.
//! Skips with no adapter.

use glam::Vec3;

use super::params_tests::{sphere, RES};
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, material as mat_ops, Primitive};
use crate::scene::{Camera, Scene};
use crate::shadergen::recipe::{BlockSel, PassKind, ShaderRecipe};
use crate::shadergen::{bake_recipe, DEFAULT_OUT_DIR, ENGINE_SHADER_DIR};

/// One baked single-block surface shader, unique to this run; removed on drop.
struct Baked(String);

impl Baked {
    fn new(block: &str) -> Self {
        let name = format!("test_{block}_{}", std::process::id());
        let recipe = ShaderRecipe {
            pass: PassKind::Surface,
            name: name.clone(),
            blocks: vec![BlockSel {
                id: block.into(),
                params: Default::default(),
            }],
        };
        bake_recipe(&recipe, ENGINE_SHADER_DIR, DEFAULT_OUT_DIR).expect("bake succeeds");
        Self(name)
    }
}

impl Drop for Baked {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.0));
        }
    }
}

/// A sun-lit, checkered sphere filling most of the view, drawn by `shader` ("" is
/// the engine's plain forward shader).
fn checkered_sphere(shader: &str) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let id = sphere(&mut scene, 0.0, shader);
    let key = mat_ops::ensure_material_key(&mut scene, id).unwrap();
    let mat = scene.materials.get_mut(&key).unwrap();
    mat.base_color_map = Some("definitely/missing_401.png".into());
    if shader.is_empty() {
        mat.shader = None;
    }
    scene
}

fn frame(renderer: &mut Renderer, scene: &mut Scene, time: f32) -> Vec<u8> {
    scene.shader_time = time;
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, RES, RES, 2);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 2.5), -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    let texture = view.color_target().unwrap();
    readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, RES, RES)
}

fn changed(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).filter(|(x, y)| x != y).count()
}

#[test]
fn gpu_gameplay_blocks_render_differ_from_plain_and_animate_with_game_time() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let plain = frame(&mut renderer, &mut checkered_sphere(""), 0.3);
    // (block, animates with game time)
    for (block, animated) in [
        ("pulse_glow", true),
        ("uv_scroll", true),
        ("triplanar_detail", false),
        ("hologram", true),
    ] {
        let baked = Baked::new(block);
        let mut scene = checkered_sphere(&baked.0);
        let a = frame(&mut renderer, &mut scene, 0.3);
        let lit: u32 = a
            .chunks(4)
            .map(|p| p[0] as u32 + p[1] as u32 + p[2] as u32)
            .sum();
        assert!(lit > 0, "{block}: black frame");
        assert!(
            changed(&plain, &a) > a.len() / 16,
            "{block}: same as the plain shader"
        );

        let again = frame(&mut renderer, &mut scene, 0.3);
        assert_eq!(
            a, again,
            "{block}: the same game time must render the same pixels"
        );
        let later = changed(&a, &frame(&mut renderer, &mut scene, 0.55));
        if animated {
            assert!(
                later > a.len() / 16,
                "{block}: did not move with time ({later})"
            );
        } else {
            assert_eq!(later, 0, "{block}: static block changed with time");
        }
    }
}

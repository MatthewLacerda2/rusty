//! Tests for the Inspector Preview tab's offscreen render path (#352): the target/
//! view contract and the shader-override fallback. Skips gracefully when no GPU/
//! software adapter is present — the same headless-test contract as
//! `render/ibl/cubemap_capture_tests.rs`.

use glam::Vec3;

use crate::components::MaterialComponent;
use crate::render::{Camera, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{DirtyFlag, MeshComponent, Scene};

const RES: u32 = 32;

/// The engine's own forward shader — a module guaranteed to compose, so the override
/// path is exercised rather than its fallback.
const SHADER: &str = "assets/shaders/shader.wgsl";

/// A preview render view (own target + depth + post-FX) sized to the test resolution.
fn preview_view(renderer: &Renderer) -> RenderView {
    RenderView::offscreen(
        &renderer.device,
        OFFSCREEN_FORMAT,
        RES,
        RES,
        renderer.quality.bloom_divisor(),
    )
}

fn sphere_scene() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let id = scene.add_entity("PreviewMesh".to_string());
    let (vertices, indices) = crate::render::gpu::mesh::generate_sphere(1.0, 8, 8);
    scene.world.set_mesh(
        id,
        Some(MeshComponent {
            primitive_type: "Sphere".to_string(),
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
    scene
        .materials
        .insert("preview::material".to_string(), Default::default());
    scene.world.set_material(
        id,
        Some(MaterialComponent {
            material: "preview::material".to_string(),
        }),
    );
    scene
}

fn looking_at_origin_from_z() -> Camera {
    Camera::new(Vec3::new(0.0, 0.0, 3.0), -90.0, 0.0)
}

#[test]
fn offscreen_preview_view_allocates_a_target_the_view_reads_back() {
    let Some(renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return; // No GPU/software adapter — skip, same contract as the screenshots.
    };
    let view = preview_view(&renderer);
    assert!(view.color_target_view().is_some());
}

#[test]
fn plain_render_into_the_preview_target_does_not_panic() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut view = preview_view(&renderer);
    let target = view.color_target_view().unwrap();
    renderer.render(
        &mut view,
        &sphere_scene(),
        &looking_at_origin_from_z(),
        &target,
        false,
        &[],
    );
}

#[test]
fn shader_override_with_an_unreadable_path_falls_back_instead_of_panicking() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut view = preview_view(&renderer);
    let target = view.color_target_view().unwrap();
    renderer.render_preview_with_shader(
        &mut view,
        &sphere_scene(),
        &looking_at_origin_from_z(),
        &target,
        "does/not/exist.wgsl",
    );
}

/// A shader override renders, and — the #355 step-4 half — belongs to the *view*, so
/// it never reaches another one. The old mutate-and-restore swapped the shared
/// `Renderer::render_pipeline` for the duration of the call, which shades the whole
/// editor with a preview module if anything in between returns early or unwinds.
///
/// Asserted in the same test as the render rather than its own: a second `Renderer`
/// is a whole device + pipelines + shadow maps, and enough concurrent ones exhaust
/// memory on a software adapter (Windows CI's WARP).
#[test]
fn a_shader_override_renders_and_stays_on_its_own_view() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let scene = sphere_scene();
    let camera = looking_at_origin_from_z();

    let mut view = preview_view(&renderer);
    let target = view.color_target_view().unwrap();
    renderer.render_preview_with_shader(&mut view, &scene, &camera, &target, SHADER);

    // A different view of the same renderer is untouched by that override.
    let mut plain = preview_view(&renderer);
    let target = plain.color_target_view().unwrap();
    renderer.render(&mut plain, &scene, &camera, &target, false, &[]);
}

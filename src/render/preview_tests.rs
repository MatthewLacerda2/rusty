//! Tests for the Inspector Preview tab's offscreen render path (#352): the target/
//! view contract and the shader-override fallback. Skips gracefully when no GPU/
//! software adapter is present — the same headless-test contract as
//! `render/ibl/cubemap_capture_tests.rs`.

use glam::Vec3;

use crate::components::MaterialComponent;
use crate::render::{Camera, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{DirtyFlag, MeshComponent, Scene};

const RES: u32 = 32;

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
    let Some(renderer) = pollster::block_on(Renderer::new_headless(RES, RES)) else {
        return; // No GPU/software adapter — skip, same contract as the screenshots.
    };
    let view = preview_view(&renderer);
    assert!(view.color_target_view().is_some());
}

#[test]
fn plain_render_into_the_preview_target_does_not_panic() {
    let Some(mut renderer) = pollster::block_on(Renderer::new_headless(RES, RES)) else {
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
    let Some(mut renderer) = pollster::block_on(Renderer::new_headless(RES, RES)) else {
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

#[test]
fn shader_override_with_the_engine_default_shader_does_not_panic() {
    let Some(mut renderer) = pollster::block_on(Renderer::new_headless(RES, RES)) else {
        return;
    };
    let mut view = preview_view(&renderer);
    let target = view.color_target_view().unwrap();
    renderer.render_preview_with_shader(
        &mut view,
        &sphere_scene(),
        &looking_at_origin_from_z(),
        &target,
        "assets/shaders/shader.wgsl",
    );
}

/// The shader override belongs to the view, not the renderer (#355 step 4). The old
/// mutate-and-restore left the *shared* forward pipeline swapped for the duration of
/// the call, so anything that returned early or unwound in between shaded the whole
/// editor with a preview module. A view-owned override cannot escape its view.
#[test]
fn the_shader_override_never_leaks_onto_another_view() {
    let Some(mut renderer) = pollster::block_on(Renderer::new_headless(32, 32)) else {
        return;
    };
    let mut preview = preview_view(&renderer);
    let mut plain = preview_view(&renderer);
    let scene = sphere_scene();
    let camera = looking_at_origin_from_z();

    let out = preview.color_target_view().unwrap();
    renderer.render_preview_with_shader(
        &mut preview,
        &scene,
        &camera,
        &out,
        "assets/shaders/shader.wgsl",
    );

    // The other view is untouched by the preview's override, and renders fine after.
    let out = plain.color_target_view().unwrap();
    renderer.render(&mut plain, &scene, &camera, &out, false, &[]);
}

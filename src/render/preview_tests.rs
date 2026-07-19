//! Tests for the Inspector Preview tab's offscreen render path (#352): the target/
//! view contract and the shader-override fallback. Skips gracefully when no GPU/
//! software adapter is present — the same headless-test contract as
//! `render/ibl/cubemap_capture_tests.rs`.

use glam::Vec3;

use crate::components::MaterialComponent;
use crate::render::{Camera, Renderer};
use crate::scene::{DirtyFlag, MeshComponent, Scene};

const RES: u32 = 32;

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
fn resize_preview_allocates_a_target_the_view_reads_back() {
    let Some(mut renderer) = pollster::block_on(Renderer::new_headless(RES, RES)) else {
        return; // No GPU/software adapter — skip, same contract as the screenshots.
    };
    assert!(renderer.preview_target_view().is_none());
    renderer.resize_preview(RES, RES);
    assert!(renderer.preview_target_view().is_some());
}

#[test]
fn plain_render_into_the_preview_target_does_not_panic() {
    let Some(mut renderer) = pollster::block_on(Renderer::new_headless(RES, RES)) else {
        return;
    };
    renderer.resize_preview(RES, RES);
    let view = renderer.preview_target_view().unwrap();
    renderer.render(
        &sphere_scene(),
        &looking_at_origin_from_z(),
        &view,
        false,
        &[],
    );
}

#[test]
fn shader_override_with_an_unreadable_path_falls_back_instead_of_panicking() {
    let Some(mut renderer) = pollster::block_on(Renderer::new_headless(RES, RES)) else {
        return;
    };
    renderer.resize_preview(RES, RES);
    let view = renderer.preview_target_view().unwrap();
    renderer.render_preview_with_shader(
        &sphere_scene(),
        &looking_at_origin_from_z(),
        &view,
        "does/not/exist.wgsl",
    );
}

#[test]
fn shader_override_with_the_engine_default_shader_does_not_panic() {
    let Some(mut renderer) = pollster::block_on(Renderer::new_headless(RES, RES)) else {
        return;
    };
    renderer.resize_preview(RES, RES);
    let view = renderer.preview_target_view().unwrap();
    renderer.render_preview_with_shader(
        &sphere_scene(),
        &looking_at_origin_from_z(),
        &view,
        "assets/shaders/shader.wgsl",
    );
}

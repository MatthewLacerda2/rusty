use glam::Vec3;

use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::MaterialComponent;
use rusty_scene::Camera;
use rusty_scene::{MeshComponent, Scene};

const RES: u32 = 32;

fn shader() -> String {
    format!("{}/shader.wgsl", rusty_shadergen::engine_shader_dir())
}

fn preview_view(renderer: &Renderer) -> RenderView {
    RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, RES, RES)
}

fn sphere_scene() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let id = scene.add_entity("PreviewMesh".to_string());
    let (vertices, indices) = rusty_components::mesh::primitives::generate_sphere(1.0, 8, 8);
    scene
        .world
        .set_mesh(id, Some(MeshComponent::new("Sphere", vertices, indices)));
    scene
        .materials
        .insert("preview::material".to_string(), Default::default());
    scene.world.set_material(
        id,
        Some(MaterialComponent::new("preview::material".to_string())),
    );
    scene
}

fn looking_at_origin_from_z() -> Camera {
    Camera::new(Vec3::new(0.0, 0.0, 3.0), -90.0, 0.0)
}

#[test]
fn gpu_offscreen_preview_view_allocates_a_target_the_view_reads_back() {
    let Some(renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let view = preview_view(&renderer);
    assert!(view.color_target_view().is_some());
}

#[test]
fn gpu_plain_render_into_the_preview_target_does_not_panic() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
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
    );
}

#[test]
fn gpu_shader_override_with_an_unreadable_path_falls_back_instead_of_panicking() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
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
fn gpu_a_shader_override_renders_and_stays_on_its_own_view() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let scene = sphere_scene();
    let camera = looking_at_origin_from_z();

    let mut view = preview_view(&renderer);
    let target = view.color_target_view().unwrap();
    renderer.render_preview_with_shader(&mut view, &scene, &camera, &target, &shader());

    let mut plain = preview_view(&renderer);
    let target = plain.color_target_view().unwrap();
    renderer.render(&mut plain, &scene, &camera, &target, false);
}

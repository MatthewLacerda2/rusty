//! Per-entity shader-param overrides on a multi-material mesh (#670, #1285): each
//! material slot owns its own param buffer, so two slots whose materials flash
//! different colours keep them under one `SetShaderParam`, frame after frame.
//! Skips with no adapter.

use super::{lua, sphere, Flash, RES};
use crate::render::gpu::material_cache::ParamOwner;
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

/// Pixels of one frame dominated by red and by blue.
fn red_blue(renderer: &mut Renderer, scene: &Scene) -> (usize, usize) {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, RES, RES);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(glam::Vec3::new(0.0, 0.0, 6.0), -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    let target = view.color_target().unwrap();
    let px = readback::read_texture_rgba8(&renderer.device, &renderer.queue, target, RES, RES);
    let dominant = |c: usize, o: usize| {
        px.chunks(4)
            .filter(|p| p[c] > 60 && u32::from(p[c]) > 3 * u32::from(p[o].max(p[1])))
            .count()
    };
    (dominant(0, 2), dominant(2, 0))
}

#[test]
fn gpu_two_slots_with_their_own_flash_colours_keep_them_under_one_override() {
    let flash = Flash::bake_as("_slots");
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let id = sphere(&mut scene, 0.0, &flash.0);
    // Two runs, two materials: the sphere's material flashes red, its copy blue.
    let red = scene.world.material(id).unwrap().first().to_string();
    let mut blue = scene.materials[&red].clone();
    blue.shader_params
        .insert("hit_flash.color".into(), vec![0.0, 0.0, 1.0]);
    scene.materials.insert("blue".into(), blue);
    scene.world.material_mut(id).unwrap().materials = vec![red, "blue".into()];
    let half = {
        let mut mesh = scene.world.mesh_mut(id).unwrap();
        let half = (mesh.indices.len() / 6 * 3) as u32;
        mesh.ranges = vec![0..half, half..mesh.indices.len() as u32];
        half
    };
    assert!(half > 0);
    let scene = std::cell::RefCell::new(scene);
    lua(
        &scene,
        &format!(r#"Material.SetShaderParam({id}, "hit_flash.amount", 1)"#),
    );

    let first = red_blue(&mut renderer, &scene.borrow());
    let second = red_blue(&mut renderer, &scene.borrow());
    eprintln!("[#1285 slot overrides] red, blue: {first:?} then {second:?}");
    assert!(
        first.0 > 0 && first.1 > 0,
        "both flash colours show: {first:?}"
    );
    assert_eq!(second, first, "each slot keeps its colour frame to frame");
    let sid = scene.borrow().id();
    let owners = [0, 1].map(|slot| ParamOwner::Entity(sid, id, slot));
    let colours = owners.map(|o| renderer.materials.params_of(&o).map(|p| p[0]));
    assert_ne!(colours[0], colours[1], "one buffer per slot: {colours:?}");
}

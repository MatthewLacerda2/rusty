//! Per-entity shader-param overrides on the GPU (#670): two spheres share one
//! material and only the overridden one flashes; clearing the override frees what it
//! owned. And the batching cost, measured: twenty enemies on one material, five
//! flashing, cost five extra draws — the fifteen others still draw as one. Skips
//! with no adapter.

use super::{halves, lua, sphere, Flash, RES};
use crate::render::{RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

/// A lit scene of `n` spheres in a row sharing one material named `shader`.
fn shared(n: u32, shader: &str) -> (Scene, Vec<u32>) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let first = sphere(&mut scene, -1.2, shader);
    let material = scene.world.material(first).map(|m| m.clone());
    let mut ids = vec![first];
    for i in 1..n {
        let x = if n == 2 { 1.2 } else { -1.2 + i as f32 * 0.4 };
        let id = sphere(&mut scene, x, shader);
        scene.world.set_material(id, material.clone());
        ids.push(id);
    }
    (scene, ids)
}

#[test]
fn gpu_an_override_flashes_one_of_two_entities_sharing_a_material() {
    let flash = Flash::bake_as("_pair");
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let (scene, ids) = shared(2, &flash.0);
    let scene = std::cell::RefCell::new(scene);
    let [l0, r0] = halves(&mut renderer, &scene.borrow());
    assert!(l0.1 > 0 && r0.1 > 0, "both lit, unflashed: {l0:?} {r0:?}");
    let groups = renderer.material_group_count();
    let buffers = renderer.materials.params_len();

    let left = ids[0];
    lua(
        &scene,
        &format!(r#"Material.SetShaderParam({left}, "hit_flash.amount", 1)"#),
    );
    let [l1, r1] = halves(&mut renderer, &scene.borrow());
    assert!(
        l1.1 < l0.1 / 4 && l1.0 > l0.0,
        "the left flashed red: {l0:?} → {l1:?}"
    );
    assert_eq!(
        r1, r0,
        "the right sphere shares the material, not the flash"
    );
    assert_eq!(
        renderer.material_group_count(),
        groups + 1,
        "one group for the override"
    );

    lua(&scene, &format!("Material.ClearShaderParam({left})"));
    assert_eq!(
        halves(&mut renderer, &scene.borrow())[0],
        l0,
        "the flash is gone"
    );
    assert_eq!(
        renderer.material_group_count(),
        groups,
        "its group was released"
    );
    assert_eq!(renderer.materials.params_len(), buffers, "and its buffer");
}

/// One frame's counters for `scene`.
fn counters(renderer: &mut Renderer, scene: &Scene) -> RenderCounters {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, RES, RES, 2);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(glam::Vec3::new(2.6, 0.0, 9.0), -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    renderer.frame_counters
}

#[test]
fn gpu_twenty_enemies_five_flashing_cost_five_draws_and_the_rest_still_batch() {
    let flash = Flash::bake_as("_twenty");
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let (scene, ids) = shared(20, &flash.0);
    let scene = std::cell::RefCell::new(scene);
    let calm = counters(&mut renderer, &scene.borrow());
    for id in &ids[..5] {
        lua(
            &scene,
            &format!(r#"Material.SetShaderParam({id}, "hit_flash.amount", 1)"#),
        );
    }
    let hit = counters(&mut renderer, &scene.borrow());
    eprintln!("#670 counters\n  calm: {calm:?}\n  5 flashing: {hit:?}");
    assert_eq!(hit.visible_entities, calm.visible_entities);
    assert_eq!(
        hit.triangles, calm.triangles,
        "the same work reaches the GPU"
    );
    // The fifteen calm enemies still share one draw; each flashing one is its own.
    // hit_flash cuts nothing, so the shadow casters are untouched.
    assert_eq!(hit.draw_calls, calm.draw_calls + 5, "{calm:?} -> {hit:?}");
    assert_eq!(hit.shadow_draws, calm.shadow_draws);
}

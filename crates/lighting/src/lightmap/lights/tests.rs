//! What a bake records it took in (#1750): the lights whose bounce it holds and the
//! meshes whose emission it holds, which SSIL then leaves out.

use super::taken_lights;
use crate::lightmap::BakeScene;
use rusty_components::{LightComponent, LightMode};
use rusty_scene::authoring::{default_light, primitive_mesh_component, Primitive};
use rusty_scene::Scene;

fn light(scene: &mut Scene, mode: LightMode, intensity: f32) -> u32 {
    let id = scene.add_entity("Light".to_string());
    let light = LightComponent {
        intensity,
        mode,
        ..default_light()
    };
    scene.world.set_light(id, Some(light));
    id
}

#[test]
fn a_bake_takes_lit_mixed_and_baked_lights_but_not_a_dark_or_realtime_one() {
    let mut scene = Scene::new();
    let mixed = light(&mut scene, LightMode::Mixed, 2.0);
    let baked = light(&mut scene, LightMode::Baked, 2.0);
    light(&mut scene, LightMode::Realtime, 2.0);
    // A flash lamp placed dark for a script to raise: the bake holds none of it.
    light(&mut scene, LightMode::Mixed, 0.0);
    assert_eq!(
        taken_lights(&scene).into_iter().collect::<Vec<_>>(),
        vec![mixed, baked]
    );
}

#[test]
fn a_bake_takes_static_meshes_with_or_without_a_lightmap() {
    let mut scene = Scene::new();
    let mesh = |scene: &mut Scene, is_static, scale| {
        let id = scene.add_entity("Box".to_string());
        let mut m = primitive_mesh_component(Primitive::Box).unwrap();
        m.scale_in_lightmap = scale;
        scene.world.set_mesh(id, Some(m));
        scene.world.set_static(id, is_static);
        id
    };
    let lightmapped = mesh(&mut scene, true, 1.0);
    let unmapped = mesh(&mut scene, true, 0.0);
    let moving = mesh(&mut scene, false, 1.0);
    assert!(BakeScene::takes_mesh(&scene, lightmapped));
    assert!(
        BakeScene::takes_mesh(&scene, unmapped),
        "it still emits in the bake"
    );
    assert!(!BakeScene::takes_mesh(&scene, moving));
    // A light's gizmo mesh is never baked.
    let gizmo = mesh(&mut scene, true, 1.0);
    scene.world.set_light(gizmo, Some(default_light()));
    assert!(!BakeScene::takes_mesh(&scene, gizmo));
}

use glam::Vec3;

use super::fixture::{eye, frame};
use crate::lod::LodSelection;
use crate::{Frustum, Renderer};
use rusty_components::{MaterialAsset, MaterialComponent, RefractionModel, RenderMode};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::Scene;

const GLASS: f32 = 0.25;
const LIQUID: f32 = 0.75;

fn clear(red: f32, refraction_model: RefractionModel) -> MaterialAsset {
    MaterialAsset {
        base_color: [red, 0.5, 0.5],
        render_mode: RenderMode::Transparent,
        alpha: 0.5,
        refraction_model,
        ..MaterialAsset::default()
    }
}

fn bottle(slots: [&str; 2]) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    rusty_scene::create::create_live_light(&mut scene, "Sun", Primitive::DirectionalLight);
    scene
        .materials
        .insert("glass".into(), clear(GLASS, RefractionModel::None));
    let liquid = clear(LIQUID, RefractionModel::Planar);
    scene.materials.insert("liquid".into(), liquid);
    let id = create_entity(&mut scene, "Bottle", Some(Primitive::Box));
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 0.0, -4.0);
    let materials = slots.map(str::to_owned).to_vec();
    scene
        .world
        .set_material(id, Some(MaterialComponent { materials }));
    scene
}

/// The red channel of each translucent draw, in the order the pass draws them.
fn drawn(r: &mut Renderer, scene: &Scene) -> Vec<f32> {
    frame(r, scene);
    let cam = eye();
    let frustum = Frustum::from_view_proj(cam.build_view_projection(1.0));
    let lod = LodSelection::for_camera(scene, &cam);
    let solids = r.precreate_solid_resources(
        scene,
        &cam,
        (&frustum, &lod),
        (false, None, Default::default()),
    );
    let table = r.retained.records.table();
    let red = |b: &crate::draw::batch::DrawBatch| table.get(b.key.record).color_tint[0];
    solids.draws.transparent.iter().map(red).collect()
}

fn priority(scene: &mut Scene, name: &str, p: i32) {
    scene
        .materials
        .get_mut(name)
        .expect("the bottle's")
        .sorting_priority = p;
}

#[test]
fn gpu_a_lower_priority_liquid_draws_before_its_glass_whatever_the_slot_order() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let mut scene = bottle(["glass", "liquid"]);
    // Equal priority keeps slot order: the liquid, drawn second, erases the glass.
    assert_eq!(drawn(&mut r, &scene), [GLASS, LIQUID]);
    priority(&mut scene, "liquid", -1);
    assert_eq!(drawn(&mut r, &scene), [LIQUID, GLASS]);
    priority(&mut scene, "liquid", 0);
    priority(&mut scene, "glass", 1);
    assert_eq!(drawn(&mut r, &scene), [LIQUID, GLASS]);
    // Past the range it is held at 50: against a liquid at 50 the two tie, so
    // slot order holds (unclamped, the glass at 9000 would draw last).
    priority(&mut scene, "liquid", 50);
    priority(&mut scene, "glass", 9000);
    assert_eq!(drawn(&mut r, &scene), [GLASS, LIQUID]);
    let scene = bottle(["liquid", "glass"]);
    assert_eq!(drawn(&mut r, &scene), [LIQUID, GLASS]);
}

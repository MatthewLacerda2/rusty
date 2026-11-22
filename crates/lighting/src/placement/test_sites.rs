use glam::Vec3;

use super::test_level::block;
use rusty_components::{MaterialAsset, MaterialComponent, RenderMode};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::Scene;

pub fn yard() -> Scene {
    let mut scene = Scene::new();
    block(
        &mut scene,
        Vec3::new(-15.0, -0.2, -15.0),
        Vec3::new(15.0, 0.0, 15.0),
    );
    scene
}

pub fn roofed_house(scene: &mut Scene) {
    let (o, i, h) = (7.0, 5.0, 4.0);
    block(scene, Vec3::new(-o, 0.0, i), Vec3::new(o, h, o));
    block(scene, Vec3::new(-o, 0.0, -i), Vec3::new(-i, h, i));
    block(scene, Vec3::new(i, 0.0, -i), Vec3::new(o, h, i));
    block(scene, Vec3::new(-o, 0.0, -o), Vec3::new(-1.0, h, -i));
    block(scene, Vec3::new(1.0, 0.0, -o), Vec3::new(o, h, -i));
    block(scene, Vec3::new(-o, h, -o), Vec3::new(o, h + 0.3, o));
}

pub fn indoor_house() -> Scene {
    let mut scene = Scene::new();
    block(
        &mut scene,
        Vec3::new(-7.0, -0.2, -7.0),
        Vec3::new(7.0, 0.0, 7.0),
    );
    roofed_house(&mut scene);
    scene
}

pub fn pool() -> Scene {
    let mut scene = Scene::new();
    let (d, r) = (-1.5, 15.0);
    block(&mut scene, Vec3::new(-r, -0.2, -r), Vec3::new(r, 0.0, -3.0));
    block(&mut scene, Vec3::new(-r, -0.2, 3.0), Vec3::new(r, 0.0, r));
    block(
        &mut scene,
        Vec3::new(-r, -0.2, -3.0),
        Vec3::new(-3.0, 0.0, 3.0),
    );
    block(
        &mut scene,
        Vec3::new(3.0, -0.2, -3.0),
        Vec3::new(r, 0.0, 3.0),
    );
    block(
        &mut scene,
        Vec3::new(-3.0, d - 0.2, -3.0),
        Vec3::new(3.0, d, 3.0),
    );
    let water = create_entity(&mut scene, "Water", Some(Primitive::Plane));
    scene.world.set_static(water, true);
    if let Some(mut t) = scene.world.transform_mut(water) {
        t.position = Vec3::new(0.0, -0.15, 0.0);
        t.scale = Vec3::new(6.0 / 15.0, 1.0, 6.0 / 15.0);
    }
    let look = MaterialAsset {
        render_mode: RenderMode::Transparent,
        alpha: 0.5,
        ..MaterialAsset::default()
    };
    scene.materials.insert("water".into(), look);
    scene
        .world
        .set_material(water, Some(MaterialComponent::new("water")));
    scene
}

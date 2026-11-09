//! A buried face is ordinary content (#1949's review): a wall standing on a floor
//! bakes with no warning, though its foot's UV island has no valid texel.

use std::cell::RefCell;

use glam::Vec3;

use super::input::BakeScene;
use super::*;
use crate::scene::lighting::occlusion::test_level::block;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

#[test]
fn a_wall_standing_on_a_floor_bakes_without_a_warning() {
    crate::logging::init();
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let floor = Vec3::new(-4.0, -0.2, -4.0);
    block(&mut scene, "Floor", floor, Vec3::new(4.0, 0.0, 4.0));
    // The wall's bottom face rests on the floor's top: buried, an island of its own.
    block(
        &mut scene,
        "Wall",
        Vec3::new(-0.1, 0.0, -2.0),
        Vec3::new(0.1, 2.0, 2.0),
    );
    let settings = BakeSettings {
        texels_per_unit: 2.0,
        samples: 4,
        bounces: 1,
        denoiser: Denoiser::None,
        ..Default::default()
    };
    let maps = bake(&BakeScene::gather(&scene, &|_| None), &settings);
    assert_eq!(maps.len(), 2, "both boxes baked a lightmap");
    let console = RefCell::new(ConsoleLogs::default());
    let said = crate::logging::forward(&console);
    let lighting: Vec<_> = said.iter().filter(|l| l.contains("[Lighting]")).collect();
    assert!(lighting.is_empty(), "the bake warned: {lighting:?}");
}

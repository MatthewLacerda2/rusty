//! `rusty_scene::decal`: an edit-mode load over a live scene drops its decals.

use glam::Vec3;

use rusty_scene::decal::DecalSpec;
use rusty_scene::Scene;

/// A scene loaded over a live one drops the decals stamped on it, as
/// Unity's scene load destroys everything of the old scene; even a persistent
/// one, which only a Play `Scene.Load` carries across.
#[test]
fn a_load_over_the_scene_drops_its_decals() {
    let path = rusty_test_support::temp::dir().join("empty.scene");
    let path = path.to_string_lossy().into_owned();
    crate::save_to_file(&Scene::new(), &path).unwrap();
    let mut scene = Scene::new();
    let spec = DecalSpec {
        persist: true,
        ..Default::default()
    };
    scene.spawn_decal(Vec3::ZERO, Vec3::Y, DecalSpec::default());
    scene.spawn_decal(Vec3::ZERO, Vec3::Y, spec);
    crate::load_from_file(&mut scene, &path).unwrap();
    assert!(scene.decals.is_empty(), "{:?}", scene.decals.len());
}

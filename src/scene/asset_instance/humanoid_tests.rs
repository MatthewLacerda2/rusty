//! A model borrowing clips follows its source (#1747): a re-written source
//! re-imports the borrowing model's meshes in the open scene.

use std::collections::BTreeSet;

use glam::Vec3;

use crate::asset::humanoid::tests::model;
use crate::core::fs::written::Stale;
use crate::scene::asset_instance::instantiate_asset;
use crate::scene::rehydrate::reimport_written;
use crate::scene::Scene;

#[test]
fn a_rewritten_source_reimports_the_models_borrowing_from_it() {
    let dir = crate::test_temp::dir().join("rusty_humanoid_follow");
    let a = model(&dir, "a.gltf", "Wave", &[]);
    let b = model(&dir, "b.gltf", "Idle", &[&a]);
    let mut scene = Scene::new();
    let reference = format!("{}::Animated", b.to_string_lossy());
    let id = instantiate_asset(&mut scene, &reference, None, Vec3::ZERO).unwrap();
    model(&dir, "a.gltf", "Lunge", &[]);
    let stale = Stale::new(vec![a.to_string_lossy().into_owned()]);
    assert_eq!(reimport_written(&mut scene, &stale, &BTreeSet::new()), 1);
    let names: Vec<_> = (scene.world.mesh(id).unwrap().clips.iter())
        .map(|c| c.name.clone())
        .collect();
    assert_eq!(names, ["Idle", "Lunge"]);
}

#[test]
fn set_humanoid_finds_a_borrower_by_another_spelling_of_its_source() {
    let dir = crate::test_temp::dir().join("rusty_humanoid_spelling");
    let a = model(&dir, "a.gltf", "Wave", &[]);
    let b = model(&dir, "b.gltf", "Idle", &[&a]);
    let mut scene = Scene::new();
    let reference = format!("{}::Animated", b.to_string_lossy());
    instantiate_asset(&mut scene, &reference, None, Vec3::ZERO).unwrap();
    let spelled = dir.join(".").join("a.gltf").to_string_lossy().into_owned();
    let mut settings = crate::asset::sidecar::load(&a).unwrap().humanoid;
    settings.bones.insert("Hips".into(), String::new());
    let set = crate::scene::asset_instance::set_humanoid(&mut scene, &spelled, settings);
    assert_eq!(set, Ok(1));
}

#[test]
fn set_humanoid_notes_the_sources_sidecar() {
    use crate::core::fs::written::written_since;
    let dir = crate::test_temp::dir().join("rusty_humanoid_note");
    let a = model(&dir, "a.gltf", "Wave", &[]);
    let mut settings = crate::asset::sidecar::load(&a).unwrap().humanoid;
    settings.bones.insert("Hips".into(), String::new());
    let (seen, _) = written_since(u64::MAX);
    let mut scene = Scene::new();
    crate::scene::asset_instance::set_humanoid(&mut scene, &a.to_string_lossy(), settings).unwrap();
    let meta = crate::asset::sidecar::meta_path(&a);
    assert!(written_since(seen)
        .1
        .contains(&meta.to_string_lossy().into_owned()));
}

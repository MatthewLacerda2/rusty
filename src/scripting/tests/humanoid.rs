//! `Assets.GetHumanoid` / `SetHumanoid` (#1747): an agent reads a model's
//! resolved humanoid map, and naming a clip source gives the scene's meshes
//! the borrowed clip at once.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use super::assets::manager;
use crate::asset::humanoid::tests::model;
use crate::scene::asset_instance::instantiate_asset;
use crate::scene::Scene;

#[test]
fn the_assets_api_reads_the_map_and_borrows_clips_into_the_scene() {
    let dir = crate::test_temp::dir().join("rusty_humanoid_api");
    let a = model(&dir, "a.gltf", "Wave", &[]);
    let b = model(&dir, "b.gltf", "Idle", &[]);
    let (a, b) = (a.to_string_lossy(), b.to_string_lossy());
    let mut raw = Scene::new();
    let id = instantiate_asset(&mut raw, &format!("{b}::Animated"), None, Vec3::ZERO).unwrap();
    let scene = Rc::new(RefCell::new(raw));
    let lua = format!(
        "local a, b = {a:?}, {b:?}
         local h = Assets.GetHumanoid(b)
         assert(h.bones.Hips == 'Joint0' and h.bones.Spine == 'Joint1', 'the sidecar map')
         assert(#h.clipsFrom == 0 and h.problems[1]:find('Head'), tostring(h.problems[1]))
         assert(Assets.SetHumanoid(b, {{ clipsFrom = {{ a }} }}) == 1)
         assert(Assets.GetHumanoid(b).clipsFrom[1] == a)
         assert(Assets.SetHumanoid(b, {{ clipsFrom = {{ a }} }}) == 0, 'unchanged')
         local ok, err = pcall(Assets.SetHumanoid, b, {{ bones = {{ Tail = 'x' }} }})
         assert(not ok and tostring(err):find('Tail'), tostring(err))
         Assets.SetHumanoid(b, {{ bones = {{ Spine = false }} }})
         assert(Assets.GetHumanoid(b).bones.Spine == nil, 'back to the guess, which finds none')"
    );
    manager(&scene).exec(&lua).unwrap();
    let scene = scene.borrow();
    let clips: Vec<_> = (scene.world.mesh(id).unwrap().clips.iter())
        .map(|c| c.name.clone())
        .collect();
    assert_eq!(clips, ["Idle", "Wave"]);
}

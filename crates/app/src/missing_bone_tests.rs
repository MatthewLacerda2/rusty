//! A frame over a rig missing a destroyed bone (#1273): the whole tick runs
//! without a panic and skins the missing slots. `scene`'s half (which slots go
//! missing) is `scene::skeleton::destroy_bone_tests`.

use std::cell::RefCell;
use std::rc::Rc;

use rusty_scene::skeleton::rig::{armature_owner, armature_skin};
use rusty_scene::Scene;

#[test]
fn a_frame_skins_a_rig_missing_a_destroyed_bone() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let bones = scene.world.mesh(rig).unwrap().skeleton.ids();
    scene.destroy_entity(bones[1]); // spine and head
    let scene = Rc::new(RefCell::new(scene));
    let mut game = super::model_writes_tests::game_over(Rc::clone(&scene));
    game.set_playing(true);
    game.tick(1.0 / 60.0);
    let palette = scene.borrow().world.mesh(rig).unwrap().pose_palette.clone();
    let bind = armature_skin().bind_palette();
    assert_eq!(palette.len(), 3);
    for (got, want) in palette.iter().zip(bind.iter()) {
        assert!(got.abs_diff_eq(*want, 1e-5), "got {got:?} want {want:?}");
    }
}

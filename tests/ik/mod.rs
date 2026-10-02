//! IK end to end (#461), through the real `GameWorld::tick` on the hitbox tests'
//! five-bone rig (`pelvis → spine → {head, arm → finger}`): a script-set target
//! is reached and skinned, the arm's hitbox follows the IK'd pose, an aim chain
//! clamps without drifting, weight 0 is an exact no-op, a ragdolled bone is left
//! to physics, and the whole path is deterministic.

mod aim;
mod api;

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::components::AnimatorComponent;
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::scene::{Scene, ScriptComponent};
use rusty::scripting::ConsoleLogs;

use crate::hitboxes::rig;

/// Within the arm's reach from the spine (0.39 + 0.35).
pub const TARGET: Vec3 = Vec3::new(0.3, 1.7, 0.3);

/// `Start` gives the rig a two-bone arm reaching for [`TARGET`].
pub const REACH: &str = "return { Start = function(id)\n\
  Animator.AddTwoBoneIK(id, 'Arm', 'spine', 'arm', 'finger')\n\
  Animator.SetIKTarget(id, 'Arm', 0.3, 1.7, 0.3)\n\
  Animator.SetIKHint(id, 'Arm', 0.3, 3, 0)\nend }";

/// The rig (with an idle Animator) running `script`, mutated by `setup`.
pub fn rig_with(script: &str, setup: impl FnOnce(&mut Scene, u32)) -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = rig::character(&mut scene, Vec3::ZERO, false);
    scene
        .world
        .set_animator(id, Some(AnimatorComponent::default()));
    let path = crate::temp::dir().join(format!("rusty_461_{:x}.lua", hash(script)));
    std::fs::write(&path, script).unwrap();
    *scene.world.scripts_mut(id).unwrap() = vec![ScriptComponent {
        path: path.to_string_lossy().replace('\\', "/"),
        ..Default::default()
    }];
    setup(&mut scene, id);
    (scene, id)
}

fn hash(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    })
}

/// Play `scene` for `ticks` fixed steps; the world is returned for its physics.
pub fn play(scene: Scene, ticks: usize) -> GameWorld {
    let mut gw = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -5.0, 5.0, -5.0, 5.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    gw.set_playing(true);
    for _ in 0..ticks {
        gw.tick(1.0 / 60.0);
    }
    gw
}

pub fn bone_at(scene: &Scene, rig: u32, name: &str) -> Vec3 {
    let bone = scene.find_bone(rig, name).unwrap();
    scene.compute_world_matrix(bone).w_axis.truncate()
}

#[test]
fn a_script_set_target_is_reached_and_the_skin_follows() {
    let (scene, rig) = rig_with(REACH, |_, _| {});
    let gw = play(scene, 5);
    let s = gw.scene().borrow();
    let finger = bone_at(&s, rig, "finger");
    assert!(finger.abs_diff_eq(TARGET, 1e-4), "finger at {finger:?}");
    assert!(
        bone_at(&s, rig, "arm").y > 1.5,
        "the elbow bent toward the hint"
    );
    // The palette is built from the IK'd bones: the finger's bind spot lands on
    // the target.
    let palette = s.world.mesh(rig).unwrap().pose_palette.clone();
    let skinned = palette[4].transform_point3(Vec3::new(0.6, 1.5, 0.0));
    assert!(skinned.abs_diff_eq(TARGET, 1e-4), "skinned at {skinned:?}");
}

/// [`REACH`], with `LateUpdate` sliding the target 0.2 along -Z every tick.
const SLIDE: &str = "local z = 0.3\n\
return { Start = function(id)\n\
  Animator.AddTwoBoneIK(id, 'Arm', 'spine', 'arm', 'finger')\nend,\n\
  LateUpdate = function(id)\n\
  z = z - 0.2\n\
  Animator.SetIKTarget(id, 'Arm', 0.3, 1.7, z)\nend }";

/// The bone a straight-down ray over `at` strikes.
fn struck(gw: &GameWorld, at: Vec3) -> Option<u32> {
    let physics = gw.resources.physics.borrow();
    let hit = physics
        .as_ref()?
        .raycast_hit(at + Vec3::Y * 2.0, -Vec3::Y, 5.0, |_| true)?;
    gw.scene().borrow().hit_bone(hit.id)
}

#[test]
fn the_arm_hitbox_follows_the_ik_pose_the_same_tick() {
    let (scene, rig) = rig_with(SLIDE, |_, _| {});
    let mut gw = play(scene, 2);
    let arm_mid = |gw: &GameWorld| {
        let s = gw.scene().borrow();
        (bone_at(&s, rig, "arm") + bone_at(&s, rig, "finger")) * 0.5
    };
    let before = arm_mid(&gw);
    gw.tick(1.0 / 60.0);
    let now = arm_mid(&gw);
    assert!(
        now.distance(before) > 0.08,
        "the arm moved: {before:?} → {now:?}"
    );
    let arm = gw.scene().borrow().find_bone(rig, "arm");
    // IK runs before `follow_bones`, so the hitbox is already where IK put it.
    assert_eq!(struck(&gw, now), arm, "the ray strikes the IK'd arm");
    assert_ne!(struck(&gw, before), arm, "not where it stood a tick ago");
}

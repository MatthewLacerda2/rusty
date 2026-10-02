//! A ragdolled skeleton is never IK'd (#461 × #466): with an IK target set, an
//! enabled ragdoll keeps exactly the pose physics gives it, while the same rig,
//! still animated, is bent by IK.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::components::AnimatorComponent;
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::scene::{Scene, ScriptComponent};
use rusty::scripting::ConsoleLogs;

use super::{ragdolled, rig};

/// `Update` enables the ragdoll on tick 2 when `enable`, then on tick 3 gives the
/// left arm a two-bone IK target above the head when `ik`, so both runs fall from
/// the same pose and only the target differs.
fn script(ik: bool, enable: bool) -> String {
    let on = "if n == 2 then Ragdoll.Enable(id) end\n";
    let add = "if n == 3 then\n\
               Animator.AddTwoBoneIK(id, 'Arm', 'mixamorig:Spine', \
               'mixamorig:LeftArm', 'mixamorig:LeftForeArm')\n\
               Animator.SetIKTarget(id, 'Arm', 0.2, 2.0, 0.3)\nend\n";
    format!(
        "local n = 0\nreturn {{ Update = function(id)\nn = n + 1\n{}{}end }}",
        if enable { on } else { "" },
        if ik { add } else { "" },
    )
}

/// Every bone's world position after 60 ticks of the floored ragdoll rig.
fn run(ik: bool, enable: bool) -> Vec<Vec3> {
    let (mut scene, id) = ragdolled();
    scene
        .world
        .set_animator(id, Some(AnimatorComponent::default()));
    let path = crate::temp::dir().join(format!("rusty_461_466_{ik}_{enable}.lua"));
    std::fs::write(&path, script(ik, enable)).unwrap();
    *scene.world.scripts_mut(id).unwrap() = vec![ScriptComponent {
        path: path.to_string_lossy().replace('\\', "/"),
        ..Default::default()
    }];
    let scene = Rc::new(RefCell::new(scene));
    let mut gw = GameWorld::new(
        scene.clone(),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -5.0, 5.0, -5.0, 5.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    gw.set_playing(true);
    for _ in 0..60 {
        gw.tick(1.0 / 60.0);
    }
    let s = scene.borrow();
    assert_eq!(s.ragdoll_enabled(id), enable);
    [
        "Hips",
        "Spine",
        "Head",
        "LeftArm",
        "LeftForeArm",
        "RightArm",
    ]
    .iter()
    .map(|b| {
        s.compute_world_matrix(rig::bone(&s, id, b))
            .w_axis
            .truncate()
    })
    .collect()
}

#[test]
fn a_ragdolled_skeleton_with_an_ik_target_keeps_the_physics_pose() {
    // Animated, the target bends the arm: IK is live on this rig.
    assert_ne!(run(true, false), run(false, false), "IK moves the arm");
    // Ragdolled, the same target changes nothing: physics wins, bit for bit.
    let physics = run(false, true);
    assert_eq!(run(true, true), physics, "IK left the ragdoll alone");
    assert!(
        physics[0].y < 0.9,
        "the body fell: hips at {:?}",
        physics[0]
    );
}

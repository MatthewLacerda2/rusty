//! The IK surface's edges (#461): refusals, persistence, and physics winning
//! over IK on a ragdolled bone.

use glam::Vec3;
use rusty::components::{AnimatorComponent, IkChain, IkConstraint, IkTarget, RigidBodyComponent};

use super::{bone_at, play, rig_with, REACH, TARGET};

/// Each call's result lands in `Sink`'s position: unknown bone, unknown
/// constraint, `GetIKWeight` after a clamped set, `RemoveIK` twice.
const EDGES: &str = "return { Start = function(id)\n\
  local a = Animator.AddTwoBoneIK(id, 'Arm', 'spine', 'nope', 'finger') and 1 or 0\n\
  local b = Animator.SetIKTarget(id, 'Ghost', 0, 0, 0) and 1 or 0\n\
  Animator.AddTwoBoneIK(id, 'Arm', 'spine', 'arm', 'finger')\n\
  Animator.SetIKWeight(id, 'Arm', 7)\n\
  local w = Animator.GetIKWeight(id, 'Arm')\n\
  local r = (Animator.RemoveIK(id, 'Arm') and 10 or 0) + (Animator.RemoveIK(id, 'Arm') and 1 or 0)\n\
  Transform.SetPosition(Scene.FindEntityByName('Sink'), a + 2 * b, w, r)\nend }";

#[test]
fn bad_names_are_refused_and_weights_clamp() {
    let (scene, _) = rig_with(EDGES, |s, _| {
        s.add_entity("Sink".to_string());
    });
    let gw = play(scene, 1);
    let s = gw.scene().borrow();
    let sink = s.find_entity_by_name("Sink").unwrap();
    let got = s.world.transform(sink).unwrap().position;
    assert_eq!(
        got,
        Vec3::new(0.0, 1.0, 10.0),
        "refusals, clamp, remove once"
    );
}

#[test]
fn constraints_save_without_their_runtime_target() {
    let mut anim = AnimatorComponent::default();
    let mut c = IkConstraint::new(
        "Arm",
        IkChain::TwoBone {
            root: "spine".into(),
            mid: "arm".into(),
            tip: "finger".into(),
        },
    );
    c.weight = 0.25;
    c.target = Some(IkTarget::Point(Vec3::ONE));
    anim.set_ik(c.clone());
    anim.set_ik(IkConstraint::aim("Look", vec!["head".into()]));
    let json = serde_json::to_string(&anim).unwrap();
    let back: AnimatorComponent = serde_json::from_str(&json).unwrap();
    c.target = None;
    assert_eq!(back.ik[0], c);
    assert_eq!(back.ik[1], IkConstraint::aim("Look", vec!["head".into()]));
    let plain = serde_json::to_string(&AnimatorComponent::default()).unwrap();
    assert!(!plain.contains("\"ik\""), "no IK, nothing saved: {plain}");
}

fn body(kinematic: bool) -> RigidBodyComponent {
    RigidBodyComponent {
        active: true,
        is_kinematic: kinematic,
        mass: 1.0,
        velocity: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        use_gravity: false,
        collision_detection: Default::default(),
    }
}

#[test]
fn a_bone_on_a_dynamic_body_is_left_to_physics() {
    for (kinematic, reached) in [(true, true), (false, false)] {
        let (scene, rig) = rig_with(REACH, |s, rig| {
            let spine = s.find_bone(rig, "spine").unwrap();
            s.world.set_rigidbody(spine, Some(body(kinematic)));
        });
        let gw = play(scene, 5);
        let s = gw.scene().borrow();
        let finger = bone_at(&s, rig, "finger");
        assert_eq!(
            finger.distance(TARGET) < 1e-3,
            reached,
            "kinematic {kinematic}: finger at {finger:?}"
        );
    }
}

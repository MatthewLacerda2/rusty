//! What `Scene::hit_react` hands the impulse to, how it cleans its options, and
//! what a full-body ragdoll tracks (#1895).

use glam::Vec3;
use rusty::scene::skeleton::{Drive, HitReact};
use rusty::scene::Scene;

use super::{at_rest, rig};

fn simulated(scene: &Scene, bone: u32) -> bool {
    !scene.world.rigidbody(bone).unwrap().is_kinematic
}

#[test]
fn the_impulse_goes_to_a_simulated_body_of_the_reacting_branch() {
    let (mut scene, rig) = crate::ragdoll::ragdolled();
    let b = |s: &Scene, n: &str| rig::bone(s, rig, n);
    let (hips, fore, right) = (
        b(&scene, "Hips"),
        b(&scene, "LeftForeArm"),
        b(&scene, "RightArm"),
    );
    // A pelvis shot: the hips never react, so a thigh below takes it.
    let thigh = Vec3::new(0.1, 0.8, 0.1);
    let to = scene.hit_react(rig, (hips, thigh), &HitReact::default(), &at_rest);
    let to = to.expect("a ragdoll");
    assert_ne!(to, hips, "a kinematic body would drop it");
    assert_eq!(
        to,
        b(&scene, "LeftUpLeg"),
        "the reacting thigh nearest the shot"
    );
    assert!(simulated(&scene, to));
    // An `up_to` branch that doesn't hold the hit: its body nearest the point.
    let opts = HitReact {
        up_to: Some(right),
        ..HitReact::default()
    };
    let at = scene.world.world_position(fore);
    let to = scene.hit_react(rig, (fore, at), &opts, &at_rest).unwrap();
    assert!(scene.ragdoll_branch(rig, right).contains(&to) && simulated(&scene, to));
    // A hit inside its own branch takes it itself.
    let to = scene.hit_react(rig, (fore, at), &HitReact::default(), &at_rest);
    assert_eq!(to, Some(fore));
}

#[test]
fn hit_react_options_never_reach_physics_unbounded() {
    let (mut scene, rig) = crate::ragdoll::ragdolled();
    let fore = rig::bone(&scene, rig, "LeftForeArm");
    let opts = HitReact {
        duration: f32::INFINITY,
        drive: Drive {
            strength: f32::INFINITY,
            damping: f32::NAN,
        },
        up_to: None,
    };
    scene.hit_react(rig, (fore, Vec3::ZERO), &opts, &at_rest);
    let drive = scene.ragdolls.drive_override(fore).unwrap();
    assert!(
        drive.strength.is_finite() && drive.damping.is_finite(),
        "{drive:?}"
    );
    scene.advance_reactions(0.5);
    scene.end_reactions();
    assert!(!simulated(&scene, fore), "the default duration ended it");
}

#[test]
fn a_full_ragdoll_tracks_only_what_a_drive_or_weight_needs() {
    let (mut scene, rig) = crate::ragdoll::ragdolled();
    scene.set_ragdoll(rig, true, at_rest).unwrap();
    assert!(
        scene.ragdolls.is_idle(),
        "a plain dead body: nothing to note"
    );
    let arm = rig::bone(&scene, rig, "LeftArm");
    let drive = Drive {
        strength: 500.0,
        damping: 45.0,
    };
    scene.set_drive(rig, arm, drive);
    assert!(
        scene.ragdolls.animated(arm).is_some(),
        "a drive needs its target"
    );
    scene.set_ragdoll(rig, false, at_rest).unwrap();
    assert!(scene.ragdolls.is_idle());
}

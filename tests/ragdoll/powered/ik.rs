//! A hit reaction on an IK'd arm (#461 × #1895): IK keeps solving on the
//! animated side of the blend, so the aim holds through the reaction and the
//! tick it ends doesn't move the hand by the IK correction.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::components::AnimatorComponent;

use super::rig;

/// The rig with a two-bone IK target over its head, sliding along +X a little
/// every tick, shot in the chest on tick 10 when `shot`: each tick's forearm
/// position and whether it was simulated.
fn aimed(ticks: usize, shot: bool) -> Vec<(Vec3, bool)> {
    let (mut scene, id) = crate::ragdoll::ragdolled();
    scene
        .world
        .set_animator(id, Some(AnimatorComponent::default()));
    let spine = rig::bone(&scene, id, "Spine");
    let fore = rig::bone(&scene, id, "LeftForeArm");
    let src = format!(
        "local n = 0\n\
         return {{ Update = function(id)\n\
           n = n + 1\n\
           if n == 1 then\n\
             Animator.AddTwoBoneIK(id, 'Arm', 'mixamorig:Spine', 'mixamorig:LeftArm', 'mixamorig:LeftForeArm')\n\
           end\n\
           Animator.SetIKTarget(id, 'Arm', 0.3 + 0.004 * n, 1.9, 0.3)\n\
           if {shot} and n == 10 then\n\
             assert(Ragdoll.HitReact(id, {spine}, 0, 0, -4, 0, 1.35, 0.1, {{ duration = 0.25 }}))\n\
           end\n\
         end }}"
    );
    crate::play::attach_script(&mut scene, id, "rusty_1895_ik.lua", &src);
    let scene = Rc::new(RefCell::new(scene));
    let mut gw = crate::play::playing(scene.clone(), 5.0);
    (0..ticks)
        .map(|_| {
            gw.tick(1.0 / 60.0);
            let s = scene.borrow();
            let sim = !s.world.rigidbody(fore).unwrap().is_kinematic;
            (s.world.world_position(fore), sim)
        })
        .collect()
}

#[test]
fn a_reaction_on_an_ik_arm_keeps_the_aim_and_ends_with_no_snap() {
    let track = aimed(40, true);
    let unhit = aimed(40, false);
    let simulated: Vec<usize> = (0..track.len()).filter(|&i| track[i].1).collect();
    let (first, last) = (simulated[0], *simulated.last().unwrap());
    let aimed = track[first - 1].0;
    let moved = (first..=last).map(|i| track[i].0.distance(aimed));
    assert!(moved.fold(0.0, f32::max) > 0.01, "the shot moved the arm");
    // The aim kept moving under the reaction: the tick it ends follows it, and
    // doesn't jump by the ground IK made up meanwhile.
    let release = track[last + 1].0.distance(track[last].0);
    assert!(release < 0.012, "the release tick doesn't jump: {release}");
    let after = track[last + 2].0.distance(unhit[last + 2].0);
    assert!(after < 1e-3, "and the hand is on its IK aim: {after}");
}

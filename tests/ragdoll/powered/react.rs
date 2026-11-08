//! `Ragdoll.HitReact` in Play: the arm flinches, eases back to the Animator with
//! no snap and ends kinematic, the same way every run; an entity without a
//! ragdoll says `false`.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::scene::skeleton::RagdollOptions;
use rusty::scene::Scene;

use super::rig;

/// The running rig, ragdoll built, shot in the left forearm on tick 5 by a
/// script; play `ticks` ticks and return each tick's forearm position relative
/// to the (running) hips, and whether its body was simulated.
fn shot(ticks: usize) -> Vec<(Vec3, bool)> {
    let mut scene = Scene::new();
    rig::floor(&mut scene);
    let rig = rig::character(&mut scene, Vec3::ZERO, true);
    scene
        .build_ragdoll(rig, &RagdollOptions::default())
        .unwrap();
    let fore = rig::bone(&scene, rig, "LeftForeArm");
    let hips = rig::bone(&scene, rig, "Hips");
    let bare = scene.add_entity("Bare".to_string());
    let src = format!(
        "local n = 0\n\
         return {{ Update = function(id)\n\
           n = n + 1\n\
           if n == 5 then\n\
             assert(Ragdoll.HitReact(id, {fore}, 0, 0, -6, 0.6, 1.45, 0, {{ duration = 0.25 }}))\n\
             assert(Ragdoll.HitReact({bare}, {bare}, 0, 0, -6, 0, 0, 0) == false)\n\
           end\n\
         end }}"
    );
    crate::play::attach_script(&mut scene, rig, "rusty_1895_react.lua", &src);
    let scene = Rc::new(RefCell::new(scene));
    let mut gw = crate::play::playing(scene.clone(), 5.0);
    let mut out = Vec::new();
    for _ in 0..ticks {
        gw.tick(1.0 / 60.0);
        let s = scene.borrow();
        let sim = !s.world.rigidbody(fore).unwrap().is_kinematic;
        let at = s.world.world_position(fore) - s.world.world_position(hips);
        out.push((at, sim));
    }
    out
}

#[test]
fn a_hit_reaction_flinches_then_eases_back_with_no_snap() {
    let track = shot(40);
    let simulated: Vec<usize> = (0..track.len()).filter(|&i| track[i].1).collect();
    let (first, last) = (simulated[0], *simulated.last().unwrap());
    assert!(
        (13..=17).contains(&(last - first)),
        "the branch reacted ~0.25 s: ticks {first}..={last}"
    );
    assert!(!track[last + 1].1, "and ended kinematic");
    let rest = track[first - 1].0;
    let off = |i: usize| track[i].0.distance(rest);
    let most = (first..=last).map(off).fold(0.0, f32::max);
    assert!(most > 0.05, "the shot knocked the forearm: {most}");
    let step = |i: usize| track[i + 1].0.distance(track[i].0);
    // After the shot's own tick, the way back is smooth.
    let jumps: Vec<f32> = (first + 1..last + 3).map(step).collect();
    assert!(jumps.iter().all(|&d| d < 0.05), "no snap: {jumps:?}");
    // The tick the reaction ends is drawn at weight 0, then leaves physics.
    assert!(off(last + 1) < 1e-4, "its last tick is drawn animated");
    assert!(off(last + 3) < 1e-4, "and the Animator holds it after");
}

#[test]
fn two_runs_of_the_same_hits_give_bit_identical_bones() {
    let a = shot(30);
    let b = shot(30);
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        assert_eq!(x.0.to_array(), y.0.to_array(), "tick {i}");
    }
}

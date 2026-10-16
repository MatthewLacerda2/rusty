//! The Animator ↔ physics switch: a running character ragdolls with the speed it
//! ran at, physics overrides the Animator while it lies there, and `Disable` hands
//! the bones back.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::scene::skeleton::{RagdollOptions, RAGDOLL_LAYER};
use rusty::scene::Scene;

use super::rig;

const SWITCH: &str = "local n = 0\n\
return { Update = function(id)\n\
  n = n + 1\n\
  if n == 20 then assert(Ragdoll.Enable(id) == 11) end\n\
  if n == 40 then Ragdoll.Disable(id) end\n\
end }";

/// The running rig, ragdoll built and the switch script attached, in Play.
fn running() -> (GameWorld, Rc<RefCell<Scene>>, u32) {
    let mut scene = Scene::new();
    let rig = rig::character(&mut scene, Vec3::ZERO, true);
    scene
        .build_ragdoll(rig, &RagdollOptions::default())
        .unwrap();
    crate::play::attach_script(&mut scene, rig, "rusty_466_switch.lua", SWITCH);
    let scene = Rc::new(RefCell::new(scene));
    let gw = crate::play::playing(scene.clone(), 5.0);
    (gw, scene, rig)
}

#[test]
fn a_running_ragdoll_keeps_its_speed_and_disable_hands_the_bones_back() {
    let (mut gw, scene, rig) = running();
    let hips = rig::bone(&scene.borrow(), rig, "Hips");
    let hitbox = scene.borrow().hitbox_of(hips).unwrap();
    let layer = |s: &Scene, name: &str| s.layers.index_of(name).unwrap();
    for _ in 0..30 {
        gw.tick(1.0 / 60.0);
    }
    {
        let s = scene.borrow();
        let rb = s.world.rigidbody(hips).unwrap().clone();
        assert!(!rb.is_kinematic && s.ragdoll_enabled(rig));
        assert!(
            (rb.velocity.z - 2.0).abs() < 0.3,
            "it fell running at the clip's 2 m/s: {}",
            rb.velocity
        );
        let y = s.world.transform(hips).unwrap().position.y;
        assert!(y < 0.95, "physics, not the clip (y 1), poses the hips: {y}");
        assert_eq!(s.world.layer(hitbox), layer(&s, RAGDOLL_LAYER));
    }
    for _ in 0..15 {
        gw.tick(1.0 / 60.0);
    }
    let s = scene.borrow();
    assert!(!s.ragdoll_enabled(rig), "Disable made every body kinematic");
    let y = s.world.transform(hips).unwrap().position.y;
    assert!(
        (y - 1.0).abs() < 1e-4,
        "the Animator poses the hips again: {y}"
    );
    assert_eq!(s.world.layer(hitbox), layer(&s, "Hitbox"));
    assert_eq!(s.world.rigidbody(hips).unwrap().velocity, Vec3::ZERO);
}

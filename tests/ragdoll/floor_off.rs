//! A ragdoll at rest on a floor whose parent goes inactive during Play falls
//! (#1707): physics syncs only what the change log names, and a parent's
//! `SetActive` names the floor below it, which the ragdoll's bodies must feel.

use rusty::scene::Scene;

use super::{hitbox_at, ragdolled};
use crate::play::{attach_script, playing, run};

/// Tick 2: ragdoll the rig. Tick 400: switch off the floor's parent.
const DROP: &str = "local n = 0\n\
return { Update = function(id)\n\
  n = n + 1\n\
  if n == 2 then Ragdoll.Enable(id) end\n\
  if n == 400 then Scene.SetActive(Scene.FindEntityByName('Level'), false) end\n\
end }";

#[test]
fn a_ragdoll_at_rest_falls_when_the_floor_under_it_is_switched_off() {
    let (mut scene, rig) = ragdolled();
    let level = scene.add_entity("Level".to_string());
    let floor = scene.world.find_by_name("Floor").expect("the rig's floor");
    scene.set_parent(floor, Some(level)).unwrap();
    attach_script(&mut scene, rig, "rusty_1707_drop.lua", DROP);
    let scene = std::rc::Rc::new(std::cell::RefCell::new(scene));
    let mut game = playing(scene.clone(), 5.0);
    run(&mut game, 399);
    let lowest = |s: &Scene| {
        let parts = s.ragdoll_bones(rig).into_iter();
        parts.map(|b| hitbox_at(s, b).y).fold(f32::MAX, f32::min)
    };
    {
        let s = scene.borrow();
        for b in s.ragdoll_bones(rig) {
            let v = s.world.rigidbody(b).unwrap().velocity;
            assert!(
                v.length() < 0.05,
                "every bone is at rest before the drop: {v}"
            );
        }
        assert!(lowest(&s) > 0.0, "it lies on the floor: {}", lowest(&s));
    }
    run(&mut game, 60);
    let low = lowest(&scene.borrow());
    assert!(
        low < -1.0,
        "with the floor gone, it fell: lowest part at {low}"
    );
}

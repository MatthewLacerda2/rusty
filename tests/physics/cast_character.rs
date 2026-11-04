//! `query_triggers = false` doesn't pass through a character (#1720): its capsule
//! replaces its Collider and stays solid, whatever that Collider's `is_trigger` says.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::components::{CharacterControllerComponent, ColliderComponent};
use rusty::scene::authoring::default_collider;
use rusty::scene::Scene;

use crate::play::{playing, run};

#[test]
fn a_cast_past_triggers_still_hits_a_character_with_a_trigger_collider() {
    let mut s = Scene::new();
    let id = s.add_entity("Enemy".to_string());
    s.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 0.0, 5.0);
    let trigger = ColliderComponent {
        is_trigger: true,
        ..default_collider()
    };
    s.world.set_collider(id, Some(trigger));
    s.world
        .set_character_controller(id, Some(CharacterControllerComponent::default()));
    let mut game = playing(Rc::new(RefCell::new(s)), 20.0);
    run(&mut game, 1);
    let line =
        "local h, hit = Physics.Raycast(0, 0, 0, 0, 0, 1, nil, nil, 50, false); return h, hit";
    let got = game.script_manager().eval(line).expect("evaluates");
    assert!(
        got.contains("true") && got.contains(&id.to_string()),
        "{got}"
    );
}

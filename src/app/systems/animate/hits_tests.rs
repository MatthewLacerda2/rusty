//! src/app/systems/animate/hits_tests.rs — a CharacterController moved by root
//! motion raises `OnControllerColliderHit` too (#1879): queued by `animate`, it
//! is dispatched with the tick's animation events.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use super::tests::{add_box, lunging};
use crate::app::game::game_tests::{world_with, DT};
use crate::components::{CharacterControllerComponent, ScriptComponent};
use crate::scene::Scene;

#[test]
fn a_lunge_into_a_wall_reports_the_wall() {
    let mut scene = Scene::new();
    let guard = lunging(&mut scene);
    let cc = CharacterControllerComponent::default();
    scene.world.set_character_controller(guard, Some(cc));
    let wall = add_box(&mut scene, Vec3::X * 1.5, Vec3::new(0.2, 4.0, 4.0));
    let path = crate::test_temp::dir().join("listener.lua");
    let code = "return { OnControllerColliderHit = function(id, hit)\n\
                HITS = (HITS or '') .. hit.id .. ';' end }";
    std::fs::write(&path, code).unwrap();
    *scene.world.scripts_mut(guard).unwrap() = vec![ScriptComponent {
        path: path.to_string_lossy().into_owned(),
        ..Default::default()
    }];
    let mut gw = world_with(Rc::new(RefCell::new(scene)));
    gw.set_playing(true);
    for _ in 0..90 {
        gw.tick(DT);
    }
    let hits = gw.script_manager().eval("HITS").unwrap();
    assert!(
        hits.split(';').any(|h| h == wall.to_string()),
        "the wall the lunge stopped at: {hits}"
    );
}

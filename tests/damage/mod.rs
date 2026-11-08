//! `Damage` and `OnTakeDamage` end to end (#1893): a shot bubbles from a hitbox
//! to the character's script, a handler's return is what `Apply` answers, a
//! barrel's blast chains to the next and a self-feeding loop stops at depth 16
//! (`chain`), and the radial falloff, cover and grouping (`radial`).

mod chain;
#[cfg(feature = "dev")]
mod content;
mod radial;

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::components::{ColliderComponent, ColliderShape};
use rusty::scene::authoring::default_collider;
use rusty::scene::Scene;

use crate::play::{attach_script, playing, run};

/// A handler that logs each delivery in the global `LOG` and takes `TAKE` (a
/// global, `nil` for the full amount).
pub const RECORDER: &str = r#"
local M = {}
function M.OnTakeDamage(id, amount, event)
  LOG = LOG or {}
  local hit = event.hit
  LOG[#LOG + 1] = { id = id, amount = amount, kind = event.kind, type = event.type,
    instigator = event.instigator, bone = hit and hit.bone_name,
    dz = hit and hit.direction.z, distance = event.distance }
  return TAKE and amount * TAKE
end
return M
"#;

/// A static box of `size` centred at `at`, running `script` when given.
pub fn static_box(scene: &mut Scene, at: Vec3, size: Vec3, script: Option<&str>) -> u32 {
    let id = scene.add_entity("Box".to_string());
    scene.world.transform_mut(id).unwrap().position = at;
    scene.world.set_static(id, true);
    let collider = ColliderComponent {
        shape: ColliderShape::Box { size },
        ..default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    if let Some(source) = script {
        attach_script(scene, id, &format!("box_{id}.lua"), source);
    }
    id
}

/// `scene` in Play, one tick in (every script awake).
pub fn play(scene: Scene) -> GameWorld {
    let mut game = playing(Rc::new(RefCell::new(scene)), 20.0);
    run(&mut game, 1);
    game
}

/// Evaluate `line` in `game`'s gameplay VM.
pub fn eval(game: &GameWorld, line: &str) -> String {
    game.script_manager().eval(line).expect("evaluates")
}

#[test]
fn a_shot_on_a_hitbox_bubbles_to_the_characters_script() {
    let mut scene = Scene::new();
    let rig = crate::hitboxes::rig::character(&mut scene, Vec3::ZERO, false);
    attach_script(&mut scene, rig, "rig.lua", RECORDER);
    let game = play(scene);
    let shot = "local hits = Physics.RaycastAll(0,1.85,-5, 0,0,1, 20) \
                local taken = Damage.ApplyPoint(hits[1], 10, 0,0,2, \
                  { type = 'bullet', instigator = 7 }) \
                local e = LOG[1] \
                return taken, #LOG, e.id, e.kind, e.type, e.instigator, e.bone, e.dz";
    assert_eq!(
        eval(&game, shot),
        format!("10, 1, {rig}, point, bullet, 7, head, 1"),
        "the head's hitbox is under a bone; the rig's script takes it, the direction unit"
    );
}

#[test]
fn a_handlers_return_is_what_apply_answers() {
    let mut scene = Scene::new();
    let target = static_box(&mut scene, Vec3::ZERO, Vec3::ONE, Some(RECORDER));
    let bare = static_box(&mut scene, Vec3::X * 5.0, Vec3::ONE, None);
    let game = play(scene);
    let line = format!(
        "local full = Damage.Apply({target}, 10) TAKE = 0.5 \
         return full, Damage.Apply({target}, 10), Damage.Apply({bare}, 10), \
         Damage.Apply(nil, 10), LOG[1].kind, LOG[2].type"
    );
    assert_eq!(eval(&game, &line), "10, 5, 0, 0, any, generic");
}

#[test]
fn a_point_impulse_pushes_the_struck_dynamic_body() {
    let mut scene = Scene::new();
    let crate_id = static_box(&mut scene, Vec3::ZERO, Vec3::ONE, None);
    scene.world.set_static(crate_id, false);
    let body = rusty::components::RigidBodyComponent {
        use_gravity: false,
        is_kinematic: false,
        ..rusty::scene::authoring::default_rigidbody()
    };
    scene.world.set_rigidbody(crate_id, Some(body));
    let game = play(scene);
    let line = "local hits = Physics.RaycastAll(0,0,-5, 0,0,1, 20) \
                local taken = Damage.ApplyPoint(hits[1], 10, 0,0,1, { impulse = 5 }) \
                local vx, vy, vz = Physics.GetVelocity(hits[1].id) \
                return taken, vz > 0";
    assert_eq!(
        eval(&game, line),
        "0, true",
        "no receiver, but the crate moves"
    );
}

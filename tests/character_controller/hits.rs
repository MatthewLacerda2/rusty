//! `OnControllerColliderHit` (#1879): raised inside `CharacterController.Move`,
//! once per collider the move touched, in sweep order, the floor included; the
//! same list on every run; and a handler moving its own controller again does
//! not recurse.

use glam::{Quat, Vec3};
use rusty::components::ScriptComponent;
use rusty::scene::Scene;

use super::{add_box, add_character, add_floor};
use crate::play::{playing, run};

/// Walks +X at 3 m/s with its own gravity, `|` before each `Move`; each hit
/// logs `id:nx,moveDirection.x,rigidbody` to the global `LOG`.
const WALKER: &str = r#"
BOUNCE = BOUNCE_SET
local W = { vy = 0 }
function W.Update(id, dt)
  LOG = (LOG or '') .. '|'
  if CharacterController.IsGrounded(id) and W.vy < 0 then W.vy = -1 end
  W.vy = W.vy - 20 * dt
  CharacterController.Move(id, 3 * dt, W.vy * dt, 0)
end
function W.OnControllerColliderHit(id, hit)
  local entry = string.format('%d:%.2f,%.2f,%s;', hit.id, hit.normal.x,
    hit.moveDirection.x, tostring(hit.rigidbody))
  LOG = LOG .. entry:gsub('%-0%.00', '0.00')
  if BOUNCE then CharacterController.Move(id, -0.01, 0, 0) end
end
return W
"#;

/// Make `sources` (file name, code) entity `id`'s scripts, in order.
pub fn attach_scripts(scene: &mut Scene, id: u32, sources: &[(&str, &str)]) {
    let scripts = sources
        .iter()
        .map(|(file, code)| {
            let path = crate::temp::dir().join(file);
            std::fs::write(&path, code).unwrap();
            ScriptComponent {
                path: path.to_string_lossy().replace('\\', "/"),
                ..Default::default()
            }
        })
        .collect();
    *scene.world.scripts_mut(id).unwrap() = scripts;
}

/// A walker 1.5 m short of a wall, played `ticks` ticks; the log, the floor's
/// and the wall's ids.
fn walk_into_wall(ticks: usize, bounce: bool) -> (String, u32, u32) {
    let mut scene = Scene::new();
    let floor = add_floor(&mut scene);
    let size = Vec3::new(1.0, 3.0, 4.0);
    let wall = add_box(&mut scene, Vec3::new(2.5, 1.5, 0.0), size, Quat::IDENTITY);
    let cc = add_character(&mut scene, Vec3::ZERO);
    let source = WALKER.replace("BOUNCE_SET", if bounce { "true" } else { "false" });
    attach_scripts(&mut scene, cc, &[("walker.lua", &source)]);
    let mut game = playing(std::rc::Rc::new(std::cell::RefCell::new(scene)), 20.0);
    run(&mut game, ticks);
    let log = game.script_manager().eval("LOG").unwrap();
    (log, floor, wall)
}

#[test]
fn each_move_hears_each_collider_once_floor_and_wall_alike() {
    let (log, floor, wall) = walk_into_wall(60, false);
    let last = log.rsplit('|').next().unwrap();
    assert!(
        last.contains(&format!("{wall}:-1.00,0.91,nil;")),
        "pressed into the wall, its normal facing back: {last}"
    );
    assert!(
        log.contains(&format!("{floor}:0.00,0.91,nil;{wall}:"))
            && log.contains(&format!("{wall}:-1.00,0.91,nil;{floor}:")),
        "the floor (gravity) and the wall in one Move, in sweep order: {log}"
    );
    for frame in log.split('|') {
        let ids: Vec<&str> = frame
            .split(';')
            .filter_map(|h| h.split(':').next())
            .collect();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            ids.len(),
            unique.len(),
            "a collider twice in one Move: {frame}"
        );
    }
}

#[test]
fn the_hits_are_the_same_on_every_run() {
    assert_eq!(walk_into_wall(60, false).0, walk_into_wall(60, false).0);
}

#[test]
fn a_handler_moving_its_own_controller_does_not_recurse() {
    let (log, _, wall) = walk_into_wall(60, true);
    let last = log.rsplit('|').next().unwrap();
    assert!(
        last.matches(&format!("{wall}:")).count() == 1,
        "the nested Move raised no hits of its own: {last}"
    );
}

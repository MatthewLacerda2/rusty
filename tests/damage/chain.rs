//! A handler that applies damage: a barrel chaining to the next, and a loop that
//! feeds itself stopping at [`rusty::api::damage::MAX_DEPTH`].

use glam::Vec3;
use rusty::scene::Scene;

use super::{eval, play, static_box, RECORDER};

/// A barrel: the first damage it takes blows it up, a 5 m blast from its centre.
const BARREL: &str = r#"
local M = {}
function M.OnTakeDamage(id, amount, event)
  EXPLODED = EXPLODED or {}
  if EXPLODED[id] then return 0 end
  EXPLODED[id] = event.kind
  local x, y, z = Transform.GetPosition(id)
  Damage.ApplyRadial(x, y, z, 50, { outer = 5, causer = id, type = "explosion" })
  return amount
end
return M
"#;

/// A target that damages itself again, forever.
const LOOP: &str = r#"
local M = {}
function M.OnTakeDamage(id, amount)
  DEPTH = (DEPTH or 0) + 1
  return Damage.Apply(id, amount)
end
return M
"#;

#[test]
fn a_shot_barrel_sets_off_the_next_one() {
    let mut scene = Scene::new();
    let first = static_box(&mut scene, Vec3::ZERO, Vec3::ONE, Some(BARREL));
    let second = static_box(&mut scene, Vec3::X * 3.0, Vec3::ONE, Some(BARREL));
    let game = play(scene);
    let line = format!(
        "local taken = Damage.Apply({first}, 1) \
         return taken, EXPLODED[{first}], EXPLODED[{second}]"
    );
    assert_eq!(eval(&game, &line), "1, any, radial");
}

#[test]
fn a_self_feeding_loop_errors_at_depth_sixteen_and_unwinds() {
    let mut scene = Scene::new();
    let looping = static_box(&mut scene, Vec3::ZERO, Vec3::ONE, Some(LOOP));
    let plain = static_box(&mut scene, Vec3::X * 5.0, Vec3::ONE, Some(RECORDER));
    let game = play(scene);
    let line = format!(
        "local ok, err = pcall(Damage.Apply, {looping}, 1) \
         return ok, DEPTH, string.find(tostring(err), 'nested deeper than 16', 1, true) ~= nil"
    );
    assert_eq!(eval(&game, &line), "false, 16, true");
    assert_eq!(
        eval(&game, &format!("return Damage.Apply({plain}, 3)")),
        "3",
        "the unwind released every level: a fresh call is depth 1"
    );
}

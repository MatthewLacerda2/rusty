//! `Damage.ApplyRadial`'s cover and candidates, past the plain wall: a blast on a
//! wall's face, a character half behind a crate, and a causer its own blast
//! leaves alone.

use glam::Vec3;
use rusty::scene::Scene;

use super::{eval, play, static_box, RECORDER};
use crate::play::attach_script;

/// The ids `call`'s result lists, space-separated.
fn ids(call: &str) -> String {
    format!(
        "local out = {{}} for _, r in ipairs({call}) do out[#out + 1] = r.id end \
         return table.concat(out, ' ')"
    )
}

#[test]
fn a_blast_on_a_walls_face_reaches_its_side_only() {
    let mut s = Scene::new();
    let front = static_box(&mut s, Vec3::Z * -1.0, Vec3::ONE, Some(RECORDER));
    let behind = static_box(&mut s, Vec3::Z * 3.0, Vec3::ONE, Some(RECORDER));
    static_box(&mut s, Vec3::Z * 1.5, Vec3::new(4.0, 4.0, 0.2), None);
    let game = play(s);
    // Forty shots from scattered starts land on the wall's face (z = 1.4); each
    // blast starts exactly at the hit point, whose last bit varies.
    let line = "local front = 0 for i = 0, 39 do \
           local x, y = (i % 7) * 0.23 - 0.7, math.floor(i / 7) * 0.21 - 0.6 \
           local h = Physics.RaycastAll(x, y, 1.0 - (i % 5) * 0.091, 0.01 * (i % 3), 0, 1, 20)[1] \
           local p = h.point \
           for _, r in ipairs(Damage.ApplyRadial(p.x, p.y, p.z, 100, { outer = 5 })) do \
             if r.id == FRONT then front = front + 1 end end \
         end return front, #LOG";
    eval(&game, &format!("FRONT = {front}"));
    assert_eq!(
        eval(&game, line),
        "40, 40",
        "every blast reaches the front target, and nothing else: not the room behind"
    );
    let _ = behind;
}

/// A character 3 m out with a crate of `height` 1.5 m in front of it; the blast
/// is low (0.3 m), so the crate's top decides what it sees.
fn crouched(height: f32) -> (rusty::app::GameWorld, u32) {
    let mut s = Scene::new();
    let rig = crate::hitboxes::rig::character(&mut s, Vec3::Z * 3.0, false);
    attach_script(&mut s, rig, "rig.lua", RECORDER);
    let size = Vec3::new(3.0, height, 0.2);
    static_box(&mut s, Vec3::new(0.0, height * 0.5, 1.5), size, None);
    (play(s), rig)
}

#[test]
fn a_character_with_its_head_over_the_crate_is_hit_once() {
    let call = "Damage.ApplyRadial(0, 0.3, 0, 100, { outer = 5 })";
    let (game, rig) = crouched(0.96);
    let line = format!("local r = {call} return #r, r[1].id, r[1].distance > 3.1, #LOG");
    assert_eq!(
        eval(&game, &line),
        format!("1, {rig}, true, 1"),
        "only the head is in view: one delivery, at the head's distance"
    );
    let (hidden, _) = crouched(1.4);
    assert_eq!(eval(&hidden, &format!("return #{call}")), "0");
}

#[test]
fn the_causer_is_neither_damaged_nor_a_shield() {
    let mut s = Scene::new();
    let grenade = static_box(&mut s, Vec3::ZERO, Vec3::splat(0.3), Some(RECORDER));
    let shell = static_box(&mut s, Vec3::Z * 0.5, Vec3::new(2.0, 2.0, 0.1), None);
    s.set_parent(shell, Some(grenade)).unwrap();
    let other = static_box(&mut s, Vec3::Z * 2.0, Vec3::ONE, Some(RECORDER));
    let game = play(s);
    let call = format!("Damage.ApplyRadial(0, 0, 0, 100, {{ outer = 5, causer = {grenade} }})");
    assert_eq!(
        eval(&game, &ids(&call)),
        other.to_string(),
        "the grenade and its child neither take the blast nor block it"
    );
    assert_eq!(eval(&game, "return #LOG"), "1");
}

//! The issue's holstered weapon (#1363): a pivot holding two skinned, animated,
//! scripted children. Deactivating the pivot stops both Animators and both scripts
//! (each gets `OnDisable`) while the children's own flags stay on; reactivating it
//! brings them back with `OnEnable`.

use std::cell::RefCell;
use std::rc::Rc;

use rusty::scene::Scene;

use super::rig::add_hero;
use crate::play::{attach_script, playing, run};

/// Each `Update` lifts its entity 1 on Y; `OnDisable` and `OnEnable` add 1 on Z.
const SCRIPT: &str = "local function bump(id, dy, dz)\n\
    local x, y, z = Transform.GetLocalPosition(id)\n\
    Transform.SetLocalPosition(id, x, y + dy, z + dz)\nend\n\
    return { Update = function(id) bump(id, 1, 0) end,\n\
    OnDisable = function(id) bump(id, 0, 1) end,\n\
    OnEnable = function(id) bump(id, 0, 1) end }";

/// `(lifted by Update, edges seen, the hand bone's X)` for each child.
fn read(scene: &Scene, parts: &[u32]) -> Vec<(f32, f32, f32)> {
    parts
        .iter()
        .map(|&p| {
            let t = scene.world.transform(p).unwrap().position;
            let hand = scene.find_bone(p, "hand").unwrap();
            (t.y, t.z, scene.world.transform(hand).unwrap().position.x)
        })
        .collect()
}

#[test]
fn an_inactive_pivot_stops_its_skinned_childrens_animators_and_scripts() {
    let mut scene = Scene::new();
    let pivot = scene.add_entity("Pivot".to_string());
    let parts: Vec<u32> = ["Shotgun", "Arms"]
        .into_iter()
        .map(|name| {
            let part = add_hero(&mut scene, name);
            attach_script(&mut scene, part, &format!("rusty_1363_{name}.lua"), SCRIPT);
            scene.set_parent(part, Some(pivot)).unwrap();
            part
        })
        .collect();
    let scene = Rc::new(RefCell::new(scene));
    let mut game = playing(scene.clone(), 5.0);
    run(&mut game, 10);
    let before = read(&scene.borrow(), &parts);
    for &(lifted, edges, x) in &before {
        assert!(lifted >= 9.0 && x > 0.0, "running: {before:?}");
        assert_eq!(edges, 1.0, "the first OnEnable: {before:?}");
    }

    scene.borrow_mut().world.set_active(pivot, false);
    run(&mut game, 10);
    let held = read(&scene.borrow(), &parts);
    for (b, h) in before.iter().zip(&held) {
        assert!(h.0 - b.0 <= 1.0, "Update stopped: {before:?} -> {held:?}");
        assert!(
            h.2 - b.2 < 0.05,
            "the Animator stopped: {before:?} -> {held:?}"
        );
        assert_eq!(h.1, 2.0, "one OnDisable each: {held:?}");
    }
    let s = scene.borrow();
    assert!(parts.iter().all(|&p| s.world.is_active_self(p)));
    assert!(parts.iter().all(|&p| !s.world.is_active_in_hierarchy(p)));
    drop(s);

    scene.borrow_mut().world.set_active(pivot, true);
    run(&mut game, 5);
    let back = read(&scene.borrow(), &parts);
    for (h, k) in held.iter().zip(&back) {
        assert!(
            k.0 > h.0 + 3.0 && k.2 > h.2,
            "running again: {held:?} -> {back:?}"
        );
        assert_eq!(k.1, 3.0, "one OnEnable each: {back:?}");
    }
}

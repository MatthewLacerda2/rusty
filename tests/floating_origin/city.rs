//! The small city placed 32 km out (its root moved there) plays, with the origin shifted
//! back, tick for tick as the same city at zero: a shift is whole steps of a power of two,
//! so it rounds no bit. The digest counts the origin, so a replay that shifts otherwise
//! diverges.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::scene::Scene;

use super::turned_on;
use crate::play::{playing, run};

const TICKS: usize = 120;

fn city_scene() -> String {
    let out = crate::temp::dir().join("origin_city");
    #[allow(
        clippy::disallowed_methods,
        reason = "generates the test game in a child"
    )]
    let made = crate::spawn::engine_bin!("game")
        .args([
            "city",
            "--param",
            "size=200",
            "--param",
            "objects=400",
            "--out",
        ])
        .arg(&out)
        .output()
        .expect("game runs");
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    out.join("assets/scenes/city.scene")
        .to_string_lossy()
        .into_owned()
}

/// Each tick's transforms and velocities, the fields the demo digest reads but the origin.
fn play(path: &str, placed: Vec3, on: bool) -> (Vec<u64>, String) {
    let mut scene = Scene::new();
    rusty::edit::load_from_file(&mut scene, path).expect("the city loads");
    for &root in &scene.world.ids().to_vec() {
        if scene.world.parent_id(root).is_none() {
            scene.world.transform_mut(root).unwrap().position += placed;
        }
    }
    let scene = Rc::new(RefCell::new(scene));
    let mut game = playing(scene.clone(), 10.0);
    if on {
        turned_on(&game);
    }
    let ticks = (0..TICKS).map(|_| {
        run(&mut game, 1);
        poses(&scene.borrow())
    });
    let ticks = ticks.collect();
    (ticks, rusty::dev::demo::state_digest(&game))
}

fn poses(scene: &Scene) -> u64 {
    let mut bytes = Vec::new();
    for &id in scene.world.ids() {
        let t = scene.world.transform(id).unwrap();
        let floats = [t.position.to_array(), t.scale.to_array()].concat();
        let floats = floats.into_iter().chain(t.rotation.to_array());
        bytes.extend(floats.flat_map(|f| f.to_bits().to_le_bytes()));
    }
    rusty::core::hash::content(&bytes)
}

#[test]
fn the_city_32_km_out_plays_as_the_city_at_zero() {
    let path = city_scene();
    let far = Vec3::new(32_768.0, 0.0, 32_768.0);
    let (zero, zero_digest) = play(&path, Vec3::ZERO, true);
    let (shifted, digest) = play(&path, far, true);
    let first = zero.iter().zip(&shifted).position(|(a, b)| a != b);
    assert_eq!(first, None, "every tick's poses match the city at zero");
    assert_ne!(digest, zero_digest, "the digest counts the origin");
    assert_eq!(
        play(&path, far, true).1,
        digest,
        "a replay shifts on the same tick"
    );
    let (unshifted, _) = play(&path, far, false);
    assert_ne!(unshifted, zero, "off, the far city stays far");
}

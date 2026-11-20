//! The firefight's shots (`scenarios/bench/firefight.lua`, #2448) cross the yard
//! as a game's do: from the muzzle, past the shooter's own hitboxes, so a cast
//! has length and a decal lands on what it hit, not at the shooter's eye.

use glam::Vec3;

use super::firefight_tests::fight;

/// A traced shot: `firefight shot r from s: <root|nothing> at d, eye x y z`.
struct Shot {
    from: String,
    root: String,
    distance: f32,
    feet: Vec3,
}

fn shots(console: &str) -> Vec<Shot> {
    let shots = console
        .lines()
        .filter_map(|l| l.split_once("firefight shot ")?.1.split_once(" from "));
    shots
        .map(|(_, rest)| {
            let (from, rest) = rest.split_once(": ").unwrap();
            let (root, rest) = rest.split_once(" at ").unwrap();
            let (d, eye) = rest.split_once(", eye ").unwrap();
            let v: Vec<f32> = eye.split(' ').map(|x| x.parse().unwrap()).collect();
            Shot {
                from: from.to_string(),
                root: root.to_string(),
                distance: d.parse().unwrap(),
                feet: Vec3::new(v[0], v[1], v[2]),
            }
        })
        .collect()
}

#[test]
fn every_shot_leaves_its_muzzle_and_lands_away_from_the_shooter() {
    let h = fight("shots", true);
    let before = h.world.borrow().scene().borrow().decals.len();
    h.step(40);
    let console = h.console_text();
    let shots = shots(&console);
    assert_eq!(shots.len(), 40 * 4, "four shots a tick: {console}");
    let hits: Vec<_> = shots.iter().filter(|s| s.root != "nothing").collect();
    assert!(!hits.is_empty(), "some shot hits something");
    for s in &hits {
        assert_ne!(s.root, s.from, "a shot never takes its own rig");
        assert!(s.distance > 1.0, "a cast crosses the yard: {}", s.distance);
    }
    let world = h.world.borrow();
    let scene = world.scene().borrow();
    let eyes: Vec<Vec3> = shots.iter().map(|s| s.feet + Vec3::Y * 1.6).collect();
    for decal in scene.decals.iter().skip(before) {
        let near = eyes
            .iter()
            .map(|e| e.distance(decal.position))
            .fold(f32::MAX, f32::min);
        assert!(near > 0.5, "a decal {near} m from a shooter's eye");
    }
}

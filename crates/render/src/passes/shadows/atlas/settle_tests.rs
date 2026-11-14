//! The size hysteresis over whole walks (#1711): a lamp approached from afar
//! reaches the largest tile, one that shrank comes back, one the budget halved
//! grows back when the pressure ends, and a want that strays, leaves and returns
//! starts counting over.

use glam::Vec3;

use super::hysteresis::SETTLE_FRAMES;
use super::persist_tests::{light, step};
use super::plan::{AtlasPlan, MAX_TILE, MIN_TILE};
use super::slots::ShadowSlots;
use crate::clusters::LocalLight;
use rusty_scene::LightType;

/// A point lamp of range 4 ten metres down -Z.
fn lamp() -> LocalLight {
    light(LightType::Point, Vec3::new(0.0, 0.0, -10.0), 1.0)
}

/// The camera inside the lamp's range (it wants [`MAX_TILE`]) and forty metres off
/// (it wants 51 texels, [`MIN_TILE`]).
const INSIDE: Vec3 = Vec3::new(0.0, 0.0, -7.0);
const FAR: Vec3 = Vec3::new(0.0, 0.0, 30.0);
/// Six metres off: it wants 341 texels, a 256² tile.
const MID: Vec3 = Vec3::new(0.0, 0.0, -4.0);

/// The size of light `i`'s tiles in `plan`.
fn size_of(plan: &AtlasPlan, i: usize) -> u32 {
    let (_, first) = plan.shadows.iter().find(|s| s.0 == i).expect("shadowed");
    plan.tiles[*first as usize].size
}

/// `frames` frames of the lone lamp seen from `eye`; the last one's tile size.
fn hold(slots: &mut ShadowSlots, eye: Vec3, frames: u32) -> u32 {
    let mut size = 0;
    for _ in 0..frames {
        size = size_of(&step(slots, &[lamp()], &[1], eye), 0);
    }
    size
}

#[test]
fn a_lamp_approached_from_afar_reaches_the_largest_tile() {
    let mut slots = ShadowSlots::default();
    assert_eq!(hold(&mut slots, FAR, 1), MIN_TILE);
    // Walk in a metre a frame to six metres off and wait there, then step inside.
    for i in 0..34 {
        step(&mut slots, &[lamp()], &[1], FAR - Vec3::Z * i as f32);
    }
    assert_eq!(hold(&mut slots, MID, SETTLE_FRAMES), MAX_TILE / 2);
    assert_eq!(hold(&mut slots, INSIDE, SETTLE_FRAMES), MAX_TILE);
}

#[test]
fn a_lamp_that_shrank_comes_back() {
    let mut slots = ShadowSlots::default();
    assert_eq!(hold(&mut slots, INSIDE, 1), MAX_TILE);
    for (eye, size) in [(MID, MAX_TILE / 2), (FAR, MIN_TILE), (MID, MAX_TILE / 2)] {
        assert_eq!(hold(&mut slots, eye, SETTLE_FRAMES), size);
    }
    assert_eq!(hold(&mut slots, INSIDE, SETTLE_FRAMES - 1), MAX_TILE / 2);
    assert_eq!(hold(&mut slots, INSIDE, 1), MAX_TILE);
}

#[test]
fn a_lamp_the_budget_halved_grows_back_when_the_pressure_ends() {
    // Three point lights the camera stands inside, each wanting six 512² faces:
    // the two bright ones take 192 of the 256 cells, the dim lamp gets 256² faces.
    let at = |x: f32, intensity| light(LightType::Point, Vec3::new(x, 0.0, -2.0), intensity);
    let crowd = [at(0.0, 0.5), at(1.0, 2.0), at(-1.0, 2.0)];
    let mut slots = ShadowSlots::default();
    for frame in 0..3 * SETTLE_FRAMES {
        let plan = step(&mut slots, &crowd, &[1, 2, 3], Vec3::ZERO);
        assert_eq!(size_of(&plan, 0), MAX_TILE / 2, "frame {frame}");
    }
    let alone = step(&mut slots, &crowd[..1], &[1], Vec3::ZERO);
    assert_eq!(size_of(&alone, 0), MAX_TILE, "grows back at once");
}

#[test]
fn a_want_that_strays_leaves_and_returns_counts_over() {
    let mut slots = ShadowSlots::default();
    assert_eq!(hold(&mut slots, INSIDE, 1), MAX_TILE);
    assert_eq!(hold(&mut slots, FAR, SETTLE_FRAMES - 1), MAX_TILE);
    // A few frames with the lamp out of the scene's lights: it is not shown.
    for _ in 0..5 {
        step(&mut slots, &[], &[], FAR);
    }
    assert_eq!(hold(&mut slots, FAR, SETTLE_FRAMES - 1), MAX_TILE);
    assert_eq!(hold(&mut slots, FAR, 1), MIN_TILE);
}

#[test]
fn a_plan_says_a_resize_is_pending_until_it_lands() {
    let mut slots = ShadowSlots::default();
    let at = |slots: &mut ShadowSlots, eye| step(slots, &[lamp()], &[1], eye).resizing;
    assert!(!at(&mut slots, INSIDE), "a new lamp takes its size at once");
    for frame in 1..SETTLE_FRAMES {
        assert!(at(&mut slots, FAR), "frame {frame} still counts");
    }
    assert!(!at(&mut slots, FAR), "the resized tile is in band");
    assert!(!at(&mut slots, FAR), "and stays so");
}

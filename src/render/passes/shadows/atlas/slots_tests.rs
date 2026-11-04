//! The slots' bookkeeping (#1711): no overlap and no leak over a long random run,
//! a despawned light or a new scene frees at once, and fragmentation repacks
//! rather than drops.

use glam::Vec3;

use super::super::persist_tests::{light, step};
use super::super::plan::{AtlasPlan, MAX_TILE};
use super::super::{Tile, ATLAS_SIZE};
use super::{Frame, ShadowSlots, KEEP_FRAMES};
use crate::render::clusters::LocalLight;
use crate::scene::{Camera, LightType, Scene};

/// Every tile inside the atlas and no two sharing a texel; the grid and the slots
/// agreeing on the cells held.
fn assert_sound(slots: &ShadowSlots, plan: &AtlasPlan) {
    let (grid, by_slots) = slots.held_cells();
    assert_eq!(grid, by_slots, "a tile leaked");
    let r = |t: &Tile| (t.origin[0], t.origin[1], t.size);
    for (i, a) in plan.tiles.iter().enumerate() {
        let (x, y, s) = r(a);
        assert!(
            x + s <= ATLAS_SIZE && y + s <= ATLAS_SIZE,
            "{:?} spills",
            r(a)
        );
        for b in &plan.tiles[i + 1..] {
            let (bx, by, bs) = r(b);
            let apart = x + s <= bx || bx + bs <= x || y + s <= by || by + bs <= y;
            assert!(apart, "{:?} and {:?} overlap", r(a), r(b));
        }
    }
}

/// A small deterministic generator: the test replays the same run every time.
fn lcg(state: &mut u64) -> u32 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1);
    (*state >> 33) as u32
}

#[test]
fn a_long_random_run_never_overlaps_nor_leaks() {
    let mut seed = 7;
    let pool: Vec<LocalLight> = (0..48)
        .map(|i| {
            let kind = [LightType::Point, LightType::Spotlight][i % 2];
            let at = Vec3::new(
                lcg(&mut seed) as f32 % 20.0 - 10.0,
                0.0,
                -2.0 - (lcg(&mut seed) % 40) as f32,
            );
            light(kind, at, 0.2 + (lcg(&mut seed) % 50) as f32 / 10.0)
        })
        .collect();
    let mut present = vec![false; pool.len()];
    let mut slots = ShadowSlots::default();
    for frame in 0..1500 {
        for _ in 0..3 {
            let i = lcg(&mut seed) as usize % pool.len();
            present[i] = !present[i];
        }
        let ids: Vec<u32> = (0..pool.len() as u32)
            .filter(|&i| present[i as usize])
            .collect();
        let lights: Vec<_> = ids.iter().map(|&i| pool[i as usize]).collect();
        let eye = Vec3::new(0.0, 0.0, (lcg(&mut seed) % 30) as f32 - 5.0);
        let plan = step(&mut slots, &lights, &ids, eye);
        assert_sound(&slots, &plan);
        let asked = super::candidates(
            &lights.iter().map(|&l| (l, true)).collect::<Vec<_>>(),
            Camera::new(eye, -90.0, 0.0).build_view_projection(16.0 / 9.0),
            eye,
        )
        .len();
        assert_eq!(
            plan.shadows.len() + plan.dropped as usize,
            asked,
            "frame {frame}"
        );
    }
    for _ in 0..=KEEP_FRAMES {
        step(&mut slots, &[], &[], Vec3::ZERO);
    }
    assert_eq!(slots.held_cells(), (0, 0), "idle lights' tiles freed");
}

#[test]
fn a_despawned_light_or_a_new_scene_frees_its_tiles_at_once() {
    let lamp = [(
        light(LightType::Point, Vec3::new(0.0, 0.0, -6.0), 1.0),
        true,
    )];
    let frame = |lights| Frame {
        lights,
        ids: &[5],
        view_proj: Camera::new(Vec3::ZERO, -90.0, 0.0).build_view_projection(1.0),
        eye: Vec3::ZERO,
    };
    let mut slots = ShadowSlots::default();
    slots.plan(None, &frame(&lamp), |_| true);
    assert_ne!(slots.held_cells().0, 0);
    let gone = Frame {
        ids: &[],
        ..frame(&[])
    };
    slots.plan(None, &gone, |_| false);
    assert_eq!(slots.held_cells(), (0, 0), "despawned: freed at once");

    slots.plan(Some(Scene::new().id()), &frame(&lamp), |_| true);
    slots.plan(Some(Scene::new().id()), &gone, |_| true);
    assert_eq!(slots.held_cells(), (0, 0), "a new scene starts over");
}

#[test]
fn fragmentation_repacks_instead_of_dropping() {
    // 200 distant spotlights fill 200 single cells, then every other one despawns:
    // 156 cells free, but no six aligned 4×4 blocks for a near point light.
    let spots: Vec<LocalLight> = (0..200)
        .map(|i| {
            light(
                LightType::Spotlight,
                Vec3::new((i % 20) as f32 - 10.0, (i / 20) as f32 - 5.0, -40.0),
                1.0,
            )
        })
        .collect();
    let ids: Vec<u32> = (0..200).collect();
    let mut slots = ShadowSlots::default();
    let first = step(&mut slots, &spots, &ids, Vec3::ZERO);
    assert_eq!(first.shadows.len(), 200);

    let near = light(LightType::Point, Vec3::new(0.0, 0.0, -2.0), 1.0);
    let mut lights: Vec<LocalLight> = spots.iter().step_by(2).copied().collect();
    let mut kept: Vec<u32> = ids.iter().step_by(2).copied().collect();
    lights.push(near);
    kept.push(1000);
    let requests: Vec<_> = lights.iter().map(|&l| (l, true)).collect();
    let frame = Frame {
        lights: &requests,
        ids: &kept,
        view_proj: Camera::new(Vec3::ZERO, -90.0, 0.0).build_view_projection(16.0 / 9.0),
        eye: Vec3::ZERO,
    };
    let plan = slots.plan(None, &frame, |id| id % 2 == 0 || id == 1000);
    assert_eq!(plan.dropped, 0, "repacked, not dropped");
    assert_eq!(plan.shadows.len(), 101);
    let (_, first_tile) = plan.shadows.iter().find(|s| s.0 == 100).unwrap();
    assert_eq!(plan.tiles[*first_tile as usize].size, MAX_TILE);
    assert_sound(&slots, &plan);
}

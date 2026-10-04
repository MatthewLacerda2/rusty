//! src/scene/default_scene/pool.rs — the pool and the jacuzzi (#747).
//!
//! The pool is a hole in the deck (`yard`): a tiled floor with a ramp at each short
//! end and a translucent water plane at deck level. The water has no collider, so a
//! character walks through it and the navmesh never treats it as ground. The jacuzzi
//! is a small raised tub on the deck, its rim too high to step over.

use glam::{Quat, Vec3};

use super::layout::*;
use super::{block, looks, place, wear, Scene};
use crate::scene::authoring::{create_entity, Primitive};

/// The pool floor (id 1), its top at `POOL_FLOOR`.
pub(super) fn add_floor(scene: &mut Scene) {
    let (hx, hz) = POOL_HALF;
    let min = Vec3::new(-hx, SLAB_BOTTOM, -hz);
    let max = Vec3::new(hx, POOL_FLOOR, hz);
    block(scene, "Pool_Floor", min, max, looks::FLOOR);
}

/// The ramps (ids 3, 4), one at each short end on the spawn axis. Each is a box
/// tilted about x so its top runs from the deck edge down to the floor; the box's
/// underside sinks into the deck and the floor, out of sight.
pub(super) fn add_ramps(scene: &mut Scene) {
    ramp(scene, "Ramp_South", -1.0);
    ramp(scene, "Ramp_North", 1.0);
}

/// The ramp at the pool's `end` (−1 for −z, +1 for +z).
fn ramp(scene: &mut Scene, name: &str, end: f32) {
    let grade = ramp_grade();
    let tilt = Quat::from_rotation_x(-end * grade.atan());
    let top_mid = Vec3::new(
        0.0,
        DECK_TOP - POOL_DEPTH * 0.5,
        end * (POOL_HALF.1 - RAMP_RUN * 0.5),
    );
    let length = RAMP_RUN * (1.0 + grade * grade).sqrt();
    let size = Vec3::new(RAMP_HALF_WIDTH * 2.0, RAMP_THICK, length);
    let centre = top_mid - tilt * Vec3::Y * (RAMP_THICK * 0.5);
    let id = block(
        scene,
        name,
        centre - size * 0.5,
        centre + size * 0.5,
        looks::FLOOR,
    );
    scene.world.transform_mut(id).unwrap().rotation = tilt;
}

/// The pool's water: a translucent plane over the whole hole, no collider.
pub(super) fn add_water(scene: &mut Scene) {
    let (hx, hz) = POOL_HALF;
    water(
        scene,
        "Pool_Water",
        Vec3::new(0.0, WATER_LEVEL, 0.0),
        (hx, hz),
    );
}

/// The jacuzzi: a basin box under a four-sided rim, and its own water inside.
pub(super) fn add_jacuzzi(scene: &mut Scene) {
    let (cx, cz) = JACUZZI_AT;
    let (h, t) = (JACUZZI_HALF, RIM_THICK);
    let at = |x: f32, y: f32, z: f32| Vec3::new(cx + x, y, cz + z);
    let basin = (at(-h, DECK_TOP, -h), at(h, BASIN_TOP, h));
    block(scene, "Jacuzzi_Basin", basin.0, basin.1, looks::FLOOR);
    let rims = [
        (
            "Jacuzzi_Rim_South",
            at(-h, DECK_TOP, -h),
            at(h, RIM_HEIGHT, t - h),
        ),
        (
            "Jacuzzi_Rim_North",
            at(-h, DECK_TOP, h - t),
            at(h, RIM_HEIGHT, h),
        ),
        (
            "Jacuzzi_Rim_West",
            at(-h, DECK_TOP, t - h),
            at(t - h, RIM_HEIGHT, h - t),
        ),
        (
            "Jacuzzi_Rim_East",
            at(h - t, DECK_TOP, t - h),
            at(h, RIM_HEIGHT, h - t),
        ),
    ];
    for (name, min, max) in rims {
        block(scene, name, min, max, looks::WALL);
    }
    let level = (BASIN_TOP + RIM_HEIGHT) * 0.5;
    water(scene, "Jacuzzi_Water", at(0.0, level, 0.0), (h - t, h - t));
}

/// A static water plane centred at `at`, `half` wide in x and z. The plane
/// primitive is 15 m square at unit scale.
fn water(scene: &mut Scene, name: &str, at: Vec3, (hx, hz): (f32, f32)) {
    let id = create_entity(scene, name, Some(Primitive::Plane));
    place(
        scene,
        id,
        at,
        Vec3::new(hx * 2.0 / 15.0, 1.0, hz * 2.0 / 15.0),
    );
    scene.world.set_static(id, true);
    wear(scene, id, looks::WATER);
}

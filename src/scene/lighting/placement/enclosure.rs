//! What the occupancy grid tells reflection placement about a room (#1737).
//!
//! * **Open sky.** Rays cast up from a capture point (straight up, and a ring at
//!   [`RING_TILT`] from vertical) either hit a ceiling or leave the level. A room
//!   where most of them leave is open air (a courtyard, a pool hall): box
//!   projection against a ceiling that isn't there bends the sky, so it gets one
//!   probe without it.
//! * **Ceiling.** Under a roof, the box's top is where the straight-up ray hits,
//!   when that is lower than the floor plan says (a glass roof, a lean-to).
//! * **Clutter.** The solid fraction of the air a room's actors move through
//!   (between a step and head height over its floor) sets how dense its probes
//!   are: a hangar full of crates gets more and smaller boxes than an empty hall.
//! * **Clearance.** A capture point keeps [`CAPTURE_CLEARANCE`] from geometry,
//!   so it never sits in a pillar or against a wall.

use glam::{Vec2, Vec3};

use super::occupancy::Occupancy;

/// How far up a ray looks for a ceiling, world units: past it is sky.
const SKY_REACH: f32 = 60.0;
/// The tilt of the ring of up rays from vertical, radians (30°).
const RING_TILT: f32 = std::f32::consts::FRAC_PI_6;
/// Rays in the ring.
const RING: usize = 8;
/// How much more densely a room is probed per unit of clutter: a room a quarter
/// full of geometry takes probes four times as narrow as an empty one.
pub const CLUTTER_GAIN: f32 = 12.0;
/// The least distance a capture point keeps from geometry, world units.
pub const CAPTURE_CLEARANCE: f32 = 0.5;
/// The body layer clutter is measured in: from a step over the floor to a head.
const BODY: (f32, f32) = (0.4, 1.8);

/// Whether most of the up rays from `at` leave the level without a hit.
pub fn open_to_sky(occ: &Occupancy, at: Vec3) -> bool {
    let (sin, cos) = RING_TILT.sin_cos();
    let ring = (0..RING).map(|k| {
        let a = k as f32 / RING as f32 * std::f32::consts::TAU;
        Vec3::new(a.cos() * sin, cos, a.sin() * sin)
    });
    let rays: Vec<Vec3> = std::iter::once(Vec3::Y).chain(ring).collect();
    let open = (rays.iter())
        .filter(|&&d| occ.ray(at, d, SKY_REACH).is_none())
        .count();
    open * 2 > rays.len()
}

/// The world `y` of the first solid straight above `at`, if one is in reach.
pub fn ceiling_over(occ: &Occupancy, at: Vec3) -> Option<f32> {
    occ.ray(at, Vec3::Y, SKY_REACH).map(|t| at.y + t)
}

/// The solid fraction of the body layer over `floor` across the XZ rectangle
/// `[lo, hi]`.
pub fn clutter(occ: &Occupancy, lo: Vec2, hi: Vec2, floor: f32) -> f32 {
    let min = Vec3::new(lo.x, floor + BODY.0, lo.y);
    let max = Vec3::new(hi.x, floor + BODY.1, hi.y);
    occ.solid_fraction(min, max)
}

/// The probe width for a room of `clutter` when an empty one takes `region`,
/// never under `least`.
pub fn cluttered_region(region: f32, clutter: f32, least: f32) -> f32 {
    (region / (1.0 + CLUTTER_GAIN * clutter.max(0.0))).max(least.min(region))
}

/// The first of `candidates` whose point keeps [`CAPTURE_CLEARANCE`], else the
/// one with the most room around it; ties keep the earlier.
pub fn clearest<T: Copy>(occ: &Occupancy, candidates: &[(T, Vec3)]) -> Option<(T, Vec3)> {
    let mut best: Option<(f32, (T, Vec3))> = None;
    for &(id, at) in candidates {
        let room = occ.clearance(at);
        if room >= CAPTURE_CLEARANCE {
            return Some((id, at));
        }
        if best.is_none_or(|(r, _)| room > r) {
            best = Some((room, (id, at)));
        }
    }
    best.map(|(_, c)| c)
}

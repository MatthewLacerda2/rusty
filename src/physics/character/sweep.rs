//! src/physics/character/sweep.rs — one `Move`'s collide-and-slide: rapier's
//! controller configured from the component, run over the live query view,
//! and the collision flags and ground normal read off what it touched.

use glam::Vec3;
use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};
use rapier3d::parry::query::ShapeCastOptions;
use rapier3d::parry::shape::Capsule;
use rapier3d::prelude::*;

use super::{capsule, CharacterMove};
use crate::components::character_controller::{COLLIDED_ABOVE, COLLIDED_BELOW, COLLIDED_SIDES};
use crate::components::CharacterControllerComponent;
use crate::physics::build::interaction_groups;
use crate::physics::query::is_live;
use crate::physics::world::PhysicsWorld;
use crate::scene::Scene;
use crate::time::FIXED_DELTA_TIME;

/// A contact normal this close to horizontal counts as the capsule's side.
const FLAG_EPS: f32 = 1e-3;
/// How far below the capsule a grounded move looks for the ground's normal,
/// beyond the skin.
const GROUND_PROBE: f32 = 0.02;
/// A floor normal with `1 - n.y` under this (within ~2.5° of straight up) is
/// near-flat: rapier's slope split (`normal × up`) is ill-conditioned there, and
/// parry's near-contact normals on a flat face scatter by about a degree.
const FLAT_FLOOR: f32 = 1e-3;

/// rapier's controller, configured from the component. `radius` is the capsule's
/// (scaled): autostep must find that much room on top of a step, so a step only
/// counts when the whole foot clears it — without it the rounded bottom would
/// hop edges taller than the step offset.
pub(in crate::physics) fn controller(
    cc: &CharacterControllerComponent,
    radius: f32,
) -> KinematicCharacterController {
    let slope = cc.slope_limit.to_radians();
    let step = (cc.step_offset > 0.0).then_some(CharacterLength::Absolute(cc.step_offset));
    KinematicCharacterController {
        offset: CharacterLength::Absolute(cc.skin_width),
        autostep: step.map(|max_height| CharacterAutostep {
            max_height,
            min_width: CharacterLength::Absolute(radius),
            include_dynamic_bodies: false,
        }),
        max_slope_climb_angle: slope,
        min_slope_slide_angle: slope,
        snap_to_ground: step,
        ..KinematicCharacterController::default()
    }
}

/// Which side of the capsule a sweep hit touched, Unity's way: below the lower
/// cap's centre is `BELOW`, above the upper cap's is `ABOVE`, the rest `SIDES`.
/// The query pipeline casts the world against the capsule, so `normal1` is the
/// obstacle's outward normal in world space and `witness2` the touched point in
/// the capsule's own (upright, unrotated) frame.
pub(in crate::physics) fn contact_flag(hit: &ShapeCastHit, half_segment: f32) -> u8 {
    let surface_up = hit.normal1.y;
    let y = hit.witness2.y;
    if surface_up > FLAG_EPS && y < -half_segment {
        COLLIDED_BELOW
    } else if surface_up < -FLAG_EPS && y > half_segment {
        COLLIDED_ABOVE
    } else {
        COLLIDED_SIDES
    }
}

impl PhysicsWorld {
    /// What a character's sweep may hit: solid, live colliders on the layers its
    /// own layer collides with, never its own body.
    pub(super) fn character_filter<'a>(
        &self,
        scene: &Scene,
        id: u32,
        live: &'a dyn Fn(ColliderHandle, &Collider) -> bool,
    ) -> QueryFilter<'a> {
        let layer = scene.world.layer(id);
        let groups = interaction_groups(layer, scene.collision_matrix.filter_mask(layer));
        let mut filter = QueryFilter::default().exclude_sensors().groups(groups);
        filter.predicate = Some(live);
        match self.id_to_body.get(&id) {
            Some(&body) => filter.exclude_rigid_body(body),
            None => filter,
        }
    }

    /// The normal of the ground just under the capsule at `pos`, if any: a short
    /// cast down past the skin, for a move that never hit the floor (walking
    /// flat).
    fn ground_below(&self, m: &MoveCtx, pos: &Pose) -> Option<Vec3> {
        let probe = ShapeCastOptions::with_max_time_of_impact(m.skin + GROUND_PROBE);
        let hit = self
            .queries(m.filter)
            .cast_shape(pos, -Vector::Y, &m.shape, probe);
        hit.and_then(|(_, h)| h.normal1.try_normalize())
    }

    /// One run of rapier's controller for `motion`, rerun without its downward
    /// part when rapier's near-flat-floor slip rule ate the walk (see [`stalled`]).
    fn pass(&self, m: &MoveCtx, motion: Vec3) -> Pass {
        let (pass, flat_hit) = self.run(m, motion);
        if motion.y < 0.0 && flat_hit && stalled(motion, &pass) {
            return self.run(m, Vec3::new(motion.x, 0.0, motion.z)).0;
        }
        pass
    }

    /// One `move_shape`, and whether it touched a floor flat to within
    /// [`FLAT_FLOOR`].
    fn run(&self, m: &MoveCtx, motion: Vec3) -> (Pass, bool) {
        let mut flags = 0;
        let mut ground = None;
        let mut flat_hit = false;
        let movement = m.controller.move_shape(
            FIXED_DELTA_TIME,
            &self.queries(m.filter),
            &m.shape,
            &m.start,
            motion,
            |hit| {
                let flag = contact_flag(&hit.hit, m.half_segment);
                if flag == COLLIDED_BELOW {
                    ground = hit.hit.normal1.try_normalize();
                    flat_hit |= hit.hit.normal1.y > 1.0 - FLAT_FLOOR;
                }
                flags |= flag;
            },
        );
        let pass = Pass {
            translation: movement.translation,
            grounded: movement.grounded,
            flags,
            ground,
        };
        (pass, flat_hit)
    }

    /// The steep slope a pass climbed, if it did. rapier lets a push into a
    /// slope steeper than the limit ride up it (it reads the push as an intent
    /// to climb); Unity's controller treats such a slope as a wall. The surface
    /// is read a radius ahead of the feet, from a step's height up: a slope is
    /// still steep there, while a step's edge gives way to its flat tread.
    fn steep_climb(&self, m: &MoveCtx, motion: Vec3, pass: &Pass) -> Option<Vec3> {
        let ahead = Vec3::new(motion.x, 0.0, motion.z).normalize_or_zero();
        if pass.translation.y <= motion.y.max(0.0) + 1e-5 || ahead == Vec3::ZERO {
            return None;
        }
        let center = m.start.translation + pass.translation;
        let feet = center - Vec3::Y * (m.half_segment + m.radius);
        let drop = m.step + m.skin + GROUND_PROBE;
        let origin = feet + ahead * m.radius + Vec3::Y * drop;
        let ray = Ray::new(origin, -Vector::Y);
        let (_, hit) = self
            .queries(m.filter)
            .cast_ray_and_get_normal(&ray, drop * 2.0, true)?;
        let n = hit.normal.try_normalize()?;
        (n.y > FLAG_EPS && !m.walkable(n)).then_some(n)
    }

    /// Run the controller for one move: the corrected translation and what it hit.
    pub(super) fn sweep(
        &self,
        scene: &Scene,
        id: u32,
        cc: &CharacterControllerComponent,
        motion: Vec3,
    ) -> Option<(Vec3, CharacterMove)> {
        let cap = capsule(scene, id, cc, cc.height)?;
        let live = |_: ColliderHandle, c: &Collider| is_live(&self.bodies, c);
        let m = MoveCtx {
            controller: controller(cc, cap.radius),
            shape: cap.shape(),
            start: Pose::from_translation(cap.center),
            half_segment: cap.half_segment,
            radius: cap.radius,
            skin: cc.skin_width,
            step: cc.step_offset,
            slope_cos: cc.slope_limit.to_radians().cos(),
            filter: self.character_filter(scene, id, &live),
        };
        let mut pass = self.pass(&m, motion);
        if let Some(steep) = self.steep_climb(&m, motion, &pass) {
            // Again, without the push into the slope: it blocks like a wall.
            pass = self.pass(&m, off_slope(motion, steep));
        }
        let end = m.start.append_translation(pass.translation);
        let below = pass.ground.or_else(|| self.ground_below(&m, &end));
        let on_ground = pass.grounded || pass.flags & COLLIDED_BELOW != 0;
        if !(on_ground || below.is_some_and(|n| m.walkable(n))) {
            return Some((pass.translation, CharacterMove::airborne(pass.flags)));
        }
        let result = CharacterMove {
            flags: pass.flags | COLLIDED_BELOW,
            grounded: true,
            ground_normal: below.unwrap_or(Vec3::Y),
        };
        Some((pass.translation, result))
    }
}

/// One move's fixed inputs: the configured controller, the capsule and where it
/// starts, and what it may hit.
struct MoveCtx<'a> {
    controller: KinematicCharacterController,
    shape: Capsule,
    start: Pose,
    half_segment: f32,
    radius: f32,
    skin: f32,
    step: f32,
    /// Cosine of the slope limit: a ground normal at least this upright is
    /// walkable.
    slope_cos: f32,
    filter: QueryFilter<'a>,
}

impl MoveCtx<'_> {
    fn walkable(&self, normal: Vec3) -> bool {
        normal.y >= self.slope_cos - 1e-4
    }
}

/// What one run of the controller produced.
struct Pass {
    translation: Vec3,
    grounded: bool,
    flags: u8,
    /// The normal of the last floor the sweep hit below the capsule.
    ground: Option<Vec3>,
}

/// Whether a pass that only touched floor lost its horizontal travel. A floor
/// never blocks walking, but rapier 0.36's controller can: against a near-flat
/// floor its "no slipping on a walkable slope" rule reads float noise in the
/// downward (gravity) part as a slip and drops the whole slide, stalling the
/// character for a tick. Without the downward part there is nothing to misread,
/// and snap-to-ground still keeps the capsule on the floor.
fn stalled(motion: Vec3, pass: &Pass) -> bool {
    let wanted = Vec3::new(motion.x, 0.0, motion.z).length();
    let got = Vec3::new(pass.translation.x, 0.0, pass.translation.z).length();
    pass.flags & (COLLIDED_SIDES | COLLIDED_ABOVE) == 0 && got < wanted * 0.99
}

/// `motion` without its horizontal push into the steep surface `normal`.
fn off_slope(motion: Vec3, normal: Vec3) -> Vec3 {
    let away = Vec3::new(normal.x, 0.0, normal.z).normalize_or_zero();
    let into = motion.dot(away);
    if into < 0.0 {
        motion - away * into
    } else {
        motion
    }
}

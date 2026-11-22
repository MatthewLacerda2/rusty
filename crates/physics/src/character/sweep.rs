use glam::Vec3;
use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};
use rapier3d::parry::query::ShapeCastOptions;
use rapier3d::parry::shape::Capsule;
use rapier3d::prelude::*;

use super::cost::{self, Work};
use super::pass::{off_slope, stalled, Pass};
use super::{capsule, CharacterMove};
use crate::query::is_live;
use crate::shape::build::interaction_groups;
use crate::world::PhysicsWorld;
use rusty_components::physics::character_controller::{
    COLLIDED_ABOVE, COLLIDED_BELOW, COLLIDED_SIDES,
};
use rusty_components::CharacterControllerComponent;
use rusty_scene::Scene;
use rusty_time::FIXED_DELTA_TIME;

// `|n.y|` past this: a floor or ceiling, not a side. Metres past the skin a downward
// probe reaches.
const FLAG_EPS: f32 = 1e-3;
const GROUND_PROBE: f32 = 0.02;
/// `1 - n.y` under this (within ~2.5° of up) is near-flat: rapier's slope split
/// (`normal × up`) is ill-conditioned there.
const FLAT_FLOOR: f32 = 1e-3;

/// `radius` is the capsule's: autostep needs that much room atop a step, else the
/// rounded bottom would hop edges taller than the step offset.
pub(crate) fn controller(
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

/// `normal1` is the obstacle's outward normal in world space, `witness2` the touched point in
/// the capsule's own (upright, unrotated) frame.
pub(crate) fn contact_flag(hit: &ShapeCastHit, half_segment: f32) -> u8 {
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

    fn ground_below(&self, m: &MoveCtx, pos: &Pose) -> Option<Vec3> {
        let probe = ShapeCastOptions::with_max_time_of_impact(m.skin + GROUND_PROBE);
        let view = self.controller_view();
        let hit = view
            .queries(m.filter)
            .cast_shape(pos, -Vector::Y, &m.shape, probe);
        hit.and_then(|(_, h)| h.normal1.try_normalize())
    }

    fn pass(&self, m: &MoveCtx, motion: Vec3) -> Pass {
        let (pass, flat_hit) = self.run(m, motion);
        if motion.y < 0.0 && flat_hit && stalled(motion, &pass) {
            let rerun = self.run(m, Vec3::new(motion.x, 0.0, motion.z)).0;
            return rerun.after(pass.collisions);
        }
        pass
    }

    fn run(&self, m: &MoveCtx, motion: Vec3) -> (Pass, bool) {
        let mut flags = 0;
        let mut ground = None;
        let mut flat_hit = false;
        let mut collisions = Vec::new();
        cost::count(Work::Run);
        let view = self.controller_view();
        let movement = m.controller.move_shape(
            FIXED_DELTA_TIME,
            &view.queries(m.filter),
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
                collisions.push(hit);
            },
        );
        let pass = Pass {
            translation: movement.translation,
            grounded: movement.grounded,
            flags,
            ground,
            collisions,
        };
        (pass, flat_hit)
    }

    /// rapier rides up a slope steeper than the limit; Unity's controller treats it as a wall.
    fn steep_climb(&self, m: &MoveCtx, motion: Vec3, pass: &Pass) -> Option<Vec3> {
        let ahead = Vec3::new(motion.x, 0.0, motion.z).normalize_or_zero();
        let steep = |n: Vec3| n.y > FLAG_EPS && !m.walkable(n);
        let touched = pass.collisions.iter().any(|c| steep(c.hit.normal1));
        if !touched || pass.translation.y <= motion.y.max(0.0) + 1e-5 || ahead == Vec3::ZERO {
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
        steep(n).then_some(n)
    }

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
            pass = self
                .pass(&m, off_slope(motion, steep))
                .after(pass.collisions);
        }
        let end = m.start.append_translation(pass.translation);
        let below = pass.ground.or_else(|| self.ground_below(&m, &end));
        let on_ground = pass.grounded || pass.flags & COLLIDED_BELOW != 0;
        let hits = self.controller_hits(&pass.collisions, motion);
        if !(on_ground || below.is_some_and(|n| m.walkable(n))) {
            let result = CharacterMove::airborne(pass.flags, hits);
            return Some((pass.translation, result));
        }
        let result = CharacterMove {
            flags: pass.flags | COLLIDED_BELOW,
            grounded: true,
            ground_normal: below.unwrap_or(Vec3::Y),
            hits,
        };
        Some((pass.translation, result))
    }
}

struct MoveCtx<'a> {
    controller: KinematicCharacterController,
    shape: Capsule,
    start: Pose,
    half_segment: f32,
    radius: f32,
    skin: f32,
    step: f32,
    slope_cos: f32,
    filter: QueryFilter<'a>,
}

impl MoveCtx<'_> {
    fn walkable(&self, normal: Vec3) -> bool {
        normal.y >= self.slope_cos - 1e-4
    }
}

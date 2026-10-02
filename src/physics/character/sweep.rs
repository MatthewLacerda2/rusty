//! src/physics/character/sweep.rs — one `Move`'s collide-and-slide: rapier's
//! controller configured from the component, run over the live query pipeline,
//! and the collision flags and ground normal read off what it touched.

use glam::Vec3;
use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};
use rapier3d::parry::query::ShapeCastOptions;
use rapier3d::prelude::*;

use super::{capsule, CharacterMove};
use crate::components::character_controller::{COLLIDED_ABOVE, COLLIDED_BELOW, COLLIDED_SIDES};
use crate::components::CharacterControllerComponent;
use crate::physics::build::interaction_groups;
use crate::physics::convert::{from_na_vec, to_na_vec};
use crate::physics::query::is_live;
use crate::physics::world::PhysicsWorld;
use crate::scene::Scene;
use crate::time::FIXED_DELTA_TIME;

/// A contact normal this close to horizontal counts as the capsule's side.
const FLAG_EPS: f32 = 1e-3;
/// How far below the capsule a grounded move looks for the ground's normal,
/// beyond the skin.
const GROUND_PROBE: f32 = 0.05;
/// The narrowest tread autostep lands on.
const STEP_MIN_WIDTH: f32 = 0.1;

/// rapier's controller, configured from the component.
pub(in crate::physics) fn controller(
    cc: &CharacterControllerComponent,
) -> KinematicCharacterController {
    let slope = cc.slope_limit.to_radians();
    let step = (cc.step_offset > 0.0).then_some(CharacterLength::Absolute(cc.step_offset));
    KinematicCharacterController {
        offset: CharacterLength::Absolute(cc.skin_width),
        autostep: step.map(|max_height| CharacterAutostep {
            max_height,
            min_width: CharacterLength::Absolute(STEP_MIN_WIDTH),
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
/// `normal1` / `witness1` are in the capsule's (upright, unrotated) frame.
pub(in crate::physics) fn contact_flag(hit: &ShapeCastHit, half_segment: f32) -> u8 {
    let surface_up = -hit.normal1.y;
    let y = hit.witness1.y;
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

    /// The normal of the ground just under a capsule at `pos`, if any: a short
    /// cast down past the skin, for a grounded move that never hit the floor
    /// (walking flat).
    fn ground_below(
        &self,
        pos: &Isometry<Real>,
        shape: &dyn Shape,
        skin: f32,
        filter: QueryFilter,
    ) -> Option<Vec3> {
        let probe = ShapeCastOptions::with_max_time_of_impact(skin + GROUND_PROBE);
        let (bodies, colliders) = (&self.bodies, &self.colliders);
        let hit = self.query_pipeline.cast_shape(
            bodies,
            colliders,
            pos,
            &-Vector::y(),
            shape,
            probe,
            filter,
        );
        hit.map(|(_, h)| -from_na_vec(h.normal1.into_inner()))
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
        let shape = cap.shape();
        let start = Isometry::translation(cap.center.x, cap.center.y, cap.center.z);
        let live = |_: ColliderHandle, c: &Collider| is_live(&self.bodies, c);
        let filter = self.character_filter(scene, id, &live);
        let mut flags = 0;
        let mut ground = None;
        let movement = controller(cc).move_shape(
            FIXED_DELTA_TIME,
            &self.bodies,
            &self.colliders,
            &self.query_pipeline,
            &shape,
            &start,
            to_na_vec(motion),
            filter,
            |hit| {
                let flag = contact_flag(&hit.hit, cap.half_segment);
                if flag == COLLIDED_BELOW {
                    ground = Some(-from_na_vec(hit.hit.normal1.into_inner()));
                }
                flags |= flag;
            },
        );
        let translation = from_na_vec(movement.translation);
        if !(movement.grounded || flags & COLLIDED_BELOW != 0) {
            return Some((translation, CharacterMove::airborne(flags)));
        }
        let end = Translation::from(movement.translation) * start;
        let ground_normal = ground
            .or_else(|| self.ground_below(&end, &shape, cc.skin_width, filter))
            .and_then(Vec3::try_normalize)
            .unwrap_or(Vec3::Y);
        let result = CharacterMove {
            flags: flags | COLLIDED_BELOW,
            grounded: true,
            ground_normal,
        };
        Some((translation, result))
    }
}

//! src/physics/character/bodies.rs — the body table the controller's sweeps
//! read (#2061): the world's bodies at their own handles, every kinematic one
//! shown as fixed. rapier's controller reads the table for two things. Riding a
//! kinematic body (`detect_grounded_status_and_apply_friction`) is what a Unity
//! controller does not do, and it stalled walks across most floors in Play (see
//! `view`); a fixed body is never ridden. Autostep's "never onto, nor through,
//! a dynamic body" (`include_dynamic_bodies: false`: the stair it would climb,
//! and `EXCLUDE_DYNAMIC` on its clearance, room and landing casts) needs the
//! dynamic bodies' types, which the copy keeps.
//!
//! The copy is cached and remade only when the table's handles or types
//! change, so a move pays one pass over the handles, not a copy of every body.
//! Nothing else in it is read: a fixed body has no ride, and autostep asks a
//! dynamic one only its type.

use std::sync::{Arc, Mutex, PoisonError};

use rapier3d::prelude::*;

/// The cached copy and the fingerprint of the table it was made from.
#[derive(Default)]
pub(in crate::physics) struct ControllerBodies(Mutex<Option<(u64, Arc<RigidBodySet>)>>);

impl ControllerBodies {
    /// The controller's copy of `bodies`, remade if `bodies` changed handles or
    /// types since the last one.
    pub(super) fn of(&self, bodies: &RigidBodySet) -> Arc<RigidBodySet> {
        let print = fingerprint(bodies);
        let mut cached = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match cached.as_ref() {
            Some((p, copy)) if *p == print => Arc::clone(copy),
            _ => {
                let copy = Arc::new(without_kinematics(bodies));
                *cached = Some((print, Arc::clone(&copy)));
                copy
            }
        }
    }
}

/// `bodies` with every kinematic body made fixed.
fn without_kinematics(bodies: &RigidBodySet) -> RigidBodySet {
    let mut copy = bodies.clone();
    for (_, body) in copy.iter_mut() {
        if body.is_kinematic() {
            body.set_body_type(RigidBodyType::Fixed, false);
        }
    }
    copy
}

/// Each body's handle and type, folded (FNV-1a): equal for two tables the
/// controller reads alike.
fn fingerprint(bodies: &RigidBodySet) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: u64| h = (h ^ v).wrapping_mul(0x0100_0000_01b3);
    mix(bodies.len() as u64);
    for (handle, body) in bodies.iter() {
        let (index, generation) = handle.into_raw_parts();
        mix(u64::from(index) << 32 | u64::from(generation));
        mix(body.body_type() as u64);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_kinematics_fixed_keeps_dynamics_and_remakes_on_a_type_change() {
        let mut bodies = RigidBodySet::new();
        let kin = bodies.insert(RigidBodyBuilder::kinematic_position_based());
        let dyn_ = bodies.insert(RigidBodyBuilder::dynamic());
        let cache = ControllerBodies::default();
        let first = cache.of(&bodies);
        assert!(first[kin].is_fixed());
        assert!(first[dyn_].is_dynamic());
        assert!(Arc::ptr_eq(&first, &cache.of(&bodies)), "unchanged: reused");
        bodies[dyn_].set_body_type(RigidBodyType::KinematicPositionBased, false);
        let second = cache.of(&bodies);
        assert!(!Arc::ptr_eq(&first, &second), "a type change remakes it");
        assert!(second[dyn_].is_fixed());
    }
}

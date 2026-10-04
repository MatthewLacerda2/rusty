//! src/physics/joints/load.rs — the load a joint carries: torque about its anchor
//! (#803) and the impulse over the whole step (#806).
//!
//! rapier's `ImpulseJoint::impulses` is not the joint's wrench. Its solver
//! orthogonalizes a joint's rows (modified Gram-Schmidt, in the bodies'
//! inverse-mass metric) with the angular locks first, so each linear row loses its
//! angular part and pushes through an inertia-weighted point near body 1's centre
//! of mass — for a joint to the world, exactly through it. The angular half is
//! then the torque about *that* point: a weight welded a metre off its pivot reads
//! zero, since gravity makes no torque about the weight's own centre.
//!
//! [`anchor_torque`] undoes that: it rebuilds the rows the way rapier 0.36 does
//! (`JointConstraintHelper`: the lock rows, then the limit rows, one group),
//! replays the orthogonalization, and sums each row's impulse times the torque it
//! applies to body 1 about the anchor. That is what Unity's `currentTorque`
//! reports and `breakTorque` is compared against. A rapier upgrade that changes
//! the rows fails the pins in `load_tests.rs`.
//!
//! **The step's impulse.** rapier writes back only the last solver substep's
//! impulse (joints are not warm-started, so each substep starts from zero). A
//! steady load is the same every substep, but a one-off one — a blast, a kick —
//! is absorbed in substep 0 and never reported. rapier 0.36 has no per-substep
//! hook, so [`step_load`] recovers the impulse over the whole step from the
//! bodies instead, without touching the sim: each lock row's impulse is the
//! velocity change the row saw beyond what gravity alone would give, over the
//! row's effective mass. The orthogonalized lock rows are mutually orthogonal in
//! the mass metric, so each row solves on its own. It counts everything that
//! moved the bodies along the locked axes, so a body with other joints or
//! contacts splits its load imperfectly; the caller keeps the larger of it and
//! the last substep's reading, which is exact for a steady load (`impulse_tests.rs`).

use glam::{Mat3, Quat, Vec3};
use rapier3d::prelude::*;

use crate::physics::collision_events::VelocitySnapshot;

/// One solver row: its Jacobian per body, the torque about the anchor it applies
/// to body 1 per unit impulse, and its impulse.
#[derive(Clone, Copy)]
struct Row {
    lin: Vec3,
    ang1: Vec3,
    ang2: Vec3,
    torque: Vec3,
    /// A limit row: rapier never orthogonalizes the other rows against it.
    bounded: bool,
    impulse: f32,
}

/// The inverse mass and world inverse inertia the solver gives a body: none
/// (immovable) for a fixed body, and for a sleeping one next to an awake partner —
/// rapier solves that joint against it as a wall. Two sleeping bodies keep the
/// impulses of their last awake step, so they keep their masses.
fn inv_mass(rb: &RigidBody, partner: &RigidBody) -> (Vec3, AngularInertia) {
    let walled = rb.is_sleeping() && !partner.is_sleeping();
    if !rb.is_dynamic_or_kinematic() || walled {
        return Default::default();
    }
    let m = rb.mass_properties();
    (m.effective_inv_mass, m.effective_world_inv_inertia)
}

/// The angular impulse `joint` applied to its first body about the joint anchor
/// in its last solver substep (world space). Zero when a body is gone.
pub(super) fn anchor_torque(joint: &ImpulseJoint, bodies: &RigidBodySet) -> Vec3 {
    let (Some(b1), Some(b2)) = (bodies.get(joint.body1()), bodies.get(joint.body2())) else {
        return Vec3::ZERO;
    };
    let rows = solved_rows(joint, b1, b2).0;
    rows.iter().map(|r| r.torque * r.impulse).sum()
}

/// The linear and angular impulse (the angular one about the anchor) `joint`
/// applied to its first body over the whole step, estimated from the bodies'
/// velocity change since `pre` (see the module docs). Lock rows only: a limit's
/// load comes from the last substep's reading. Zero when a body is gone.
pub(super) fn step_load(
    joint: &ImpulseJoint,
    bodies: &RigidBodySet,
    pre: &VelocitySnapshot,
    gravity: Vec3,
    dt: f32,
) -> (Vec3, Vec3) {
    let (Some(b1), Some(b2)) = (bodies.get(joint.body1()), bodies.get(joint.body2())) else {
        return (Vec3::ZERO, Vec3::ZERO);
    };
    // The velocity change no gravity explains; none for a body the solver could
    // not move (fixed, kinematic, asleep: rapier integrates none of them).
    let surplus = |h: RigidBodyHandle, rb: &RigidBody| match pre.get(&h) {
        Some(v) if rb.is_dynamic() && !rb.is_sleeping() => {
            let free = v.lin + gravity * rb.gravity_scale() * dt;
            (rb.linvel() - free, rb.angvel() - v.ang)
        }
        _ => (Vec3::ZERO, Vec3::ZERO),
    };
    let ((dv1, dw1), (dv2, dw2)) = (surplus(joint.body1(), b1), surplus(joint.body2(), b2));
    let (rows, metric) = solved_rows(joint, b1, b2);
    let mut load = (Vec3::ZERO, Vec3::ZERO);
    for r in rows.iter().filter(|r| !r.bounded) {
        let norm = metric.dot(r, r);
        if norm == 0.0 {
            continue;
        }
        // rapier's row velocity is lin·(v2 − v1) + ang2·w2 − ang1·w1, and an
        // impulse λ on it changes that by −λ·|row|²; so λ = −Δ / |row|².
        let seen = r.lin.dot(dv2 - dv1) + r.ang2.dot(dw2) - r.ang1.dot(dw1);
        let impulse = -seen / norm;
        load.0 += r.lin * impulse;
        load.1 += r.torque * impulse;
    }
    load
}

/// `joint`'s rows orthogonalized as rapier's solver sees them, with the metric
/// they are orthogonal in.
fn solved_rows(joint: &ImpulseJoint, b1: &RigidBody, b2: &RigidBody) -> (Vec<Row>, Metric) {
    let mut rows = rows(joint, b1, b2);
    let metric = Metric::new(inv_mass(b1, b2), inv_mass(b2, b1));
    orthogonalize(&mut rows, &metric);
    (rows, metric)
}

/// The bodies' inverse-mass metric the solver orthogonalizes rows in.
struct Metric {
    im: Vec3,
    ii1: AngularInertia,
    ii2: AngularInertia,
}

impl Metric {
    fn new((im1, ii1): (Vec3, AngularInertia), (im2, ii2): (Vec3, AngularInertia)) -> Self {
        let im = im1 + im2;
        Self { im, ii1, ii2 }
    }

    fn dot(&self, a: &Row, b: &Row) -> f32 {
        a.lin.dot(self.im * b.lin)
            + self.ii1.mul_vec(a.ang1).dot(b.ang1)
            + self.ii2.mul_vec(a.ang2).dot(b.ang2)
    }
}

/// `joint`'s solver rows in rapier's order — angular locks, linear locks, angular
/// limits, linear limits — before orthogonalization.
fn rows(joint: &ImpulseJoint, b1: &RigidBody, b2: &RigidBody) -> Vec<Row> {
    let data = &joint.data;
    let f1 = *b1.position() * data.local_frame1;
    let f2 = *b2.position() * data.local_frame2;
    let basis = Mat3::from_quat(f1.rotation);
    let locked = data.locked_axes.bits();
    let limited = data.limit_axes.bits() & !locked;
    let set = |mask: u8, i: usize| mask & (1 << i) != 0;
    // Body 1's share of the linear rows acts at frame 2's origin, snapped back
    // onto frame 1 along the locked axes: that point is the anchor.
    let lin_err = f2.translation - f1.translation;
    let anchor = (0..3)
        .filter(|&i| set(locked, i))
        .fold(f2.translation, |c, i| {
            c - basis.col(i) * lin_err.dot(basis.col(i))
        });
    let r1 = anchor - b1.center_of_mass();
    let r2 = f2.translation - b2.center_of_mass();
    let ang_basis = ang_basis(f1.rotation, f2.rotation);
    let lin_row = |i: usize, bounded, impulse| Row {
        lin: basis.col(i),
        ang1: r1.cross(basis.col(i)),
        ang2: r2.cross(basis.col(i)),
        torque: Vec3::ZERO, // a force at the anchor has no moment about it
        bounded,
        impulse,
    };
    let ang_row = |axis: Vec3, bounded, impulse| Row {
        lin: Vec3::ZERO,
        ang1: axis,
        ang2: axis,
        torque: axis,
        bounded,
        impulse,
    };
    let locks = (3..6)
        .filter(|&i| set(locked, i))
        .map(|i| ang_row(ang_basis.col(i - 3), false, joint.impulses[i]))
        .chain(
            (0..3)
                .filter(|&i| set(locked, i))
                .map(|i| lin_row(i, false, joint.impulses[i])),
        );
    let limits = (3..6)
        .filter(|&i| set(limited, i))
        .map(|i| ang_row(basis.col(i - 3), true, data.limits[i].impulse))
        .chain(
            (0..3)
                .filter(|&i| set(limited, i))
                .map(|i| lin_row(i, true, data.limits[i].impulse)),
        );
    locks.chain(limits).collect()
}

/// rapier's `diff_conj1_2_tr(q1, q2)`, sign-matched to the shorter arc: the
/// angular lock rows' axes (half the joint frame's axes at rest).
fn ang_basis(q1: Quat, q2: Quat) -> Mat3 {
    let (v1, v2, w1, w2) = (q1.xyz(), q2.xyz(), q1.w, q2.w);
    let outer = Mat3::from_cols(v1 * v2.x, v1 * v2.y, v1 * v2.z);
    let diff = (outer + Mat3::from_diagonal(Vec3::splat(w1 * w2)) - cross_mat(v1 * w2 + v2 * w1)
        + cross_mat(v1) * cross_mat(v2))
        * 0.5;
    diff.transpose() * 1f32.copysign(q1.dot(q2))
}

/// The matrix `[v]×` with `[v]× w = v × w`.
fn cross_mat(v: Vec3) -> Mat3 {
    Mat3::from_cols(
        Vec3::new(0.0, v.z, -v.y),
        Vec3::new(-v.z, 0.0, v.x),
        Vec3::new(v.y, -v.x, 0.0),
    )
}

/// rapier's `finalize_constraints`: modified Gram-Schmidt in the inverse-mass
/// metric, skipping bounded rows as a basis, carrying each row's torque along.
fn orthogonalize(rows: &mut [Row], metric: &Metric) {
    let dot = |a: &Row, b: &Row| metric.dot(a, b);
    for j in 0..rows.len() {
        let rj = rows[j];
        let dot_jj = dot(&rj, &rj);
        if rj.bounded || dot_jj == 0.0 {
            continue;
        }
        for ri in &mut rows[j + 1..] {
            let c = dot(ri, &rj) / dot_jj;
            ri.lin -= rj.lin * c;
            ri.ang1 -= rj.ang1 * c;
            ri.ang2 -= rj.ang2 * c;
            ri.torque -= rj.torque * c;
        }
    }
}

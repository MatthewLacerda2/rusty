//! src/physics/joints/load.rs — the torque a joint carries about its anchor (#803).
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

use glam::{Mat3, Quat, Vec3};
use rapier3d::prelude::*;

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
    let (r1, r2) = (
        anchor - b1.center_of_mass(),
        f2.translation - b2.center_of_mass(),
    );
    let ang_basis = ang_basis(f1.rotation, f2.rotation);
    let lin_row = |i: usize, bounded, impulse| {
        let a = basis.col(i);
        let (ang1, ang2) = (r1.cross(a), r2.cross(a));
        let torque = Vec3::ZERO; // a force at the anchor has no moment about it
        Row {
            lin: a,
            ang1,
            ang2,
            torque,
            bounded,
            impulse,
        }
    };
    let ang_row = |axis: Vec3, bounded, impulse| Row {
        lin: Vec3::ZERO,
        ang1: axis,
        ang2: axis,
        torque: axis,
        bounded,
        impulse,
    };
    let mut rows = Vec::with_capacity(12);
    for i in (3..6).filter(|&i| set(locked, i)) {
        rows.push(ang_row(ang_basis.col(i - 3), false, joint.impulses[i]));
    }
    for i in (0..3).filter(|&i| set(locked, i)) {
        rows.push(lin_row(i, false, joint.impulses[i]));
    }
    for i in (3..6).filter(|&i| set(limited, i)) {
        rows.push(ang_row(basis.col(i - 3), true, data.limits[i].impulse));
    }
    for i in (0..3).filter(|&i| set(limited, i)) {
        rows.push(lin_row(i, true, data.limits[i].impulse));
    }
    orthogonalize(&mut rows, inv_mass(b1, b2), inv_mass(b2, b1));
    rows.iter().map(|r| r.torque * r.impulse).sum()
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
fn orthogonalize(
    rows: &mut [Row],
    (im1, ii1): (Vec3, AngularInertia),
    (im2, ii2): (Vec3, AngularInertia),
) {
    let im = im1 + im2;
    let dot = |a: &Row, b: &Row| {
        a.lin.dot(im * b.lin) + ii1.mul_vec(a.ang1).dot(b.ang1) + ii2.mul_vec(a.ang2).dot(b.ang2)
    };
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

//! rapier's `ImpulseJoint::impulses` is not the joint's wrench. Its solver
//! orthogonalizes a joint's rows (modified Gram-Schmidt, in the bodies'
//! inverse-mass metric) with the angular locks first, so each linear row loses its
//! angular part and pushes through an inertia-weighted point near body 1's centre
//! of mass — for a joint to the world, exactly through it. The angular half is
//! then the torque about *that* point: a weight welded a metre off its pivot reads
//! zero, since gravity makes no torque about the weight's own centre.

use glam::{Mat3, Quat, Vec3};
use rapier3d::prelude::*;

use crate::events::collision::VelocitySnapshot;

#[derive(Clone, Copy)]
struct Row {
    lin: Vec3,
    ang1: Vec3,
    ang2: Vec3,
    torque: Vec3,
    bounded: bool,
    impulse: f32,
}

fn inv_mass(rb: &RigidBody, partner: &RigidBody) -> (Vec3, AngularInertia) {
    let walled = rb.is_sleeping() && !partner.is_sleeping();
    if !rb.is_dynamic_or_kinematic() || walled {
        return Default::default();
    }
    let m = rb.mass_properties();
    (m.effective_inv_mass, m.effective_world_inv_inertia)
}

pub(super) fn anchor_torque(joint: &ImpulseJoint, bodies: &RigidBodySet) -> Vec3 {
    let (Some(b1), Some(b2)) = (bodies.get(joint.body1()), bodies.get(joint.body2())) else {
        return Vec3::ZERO;
    };
    let rows = solved_rows(joint, b1, b2).0;
    rows.iter().map(|r| r.torque * r.impulse).sum()
}

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
        let seen = r.lin.dot(dv2 - dv1) + r.ang2.dot(dw2) - r.ang1.dot(dw1);
        let impulse = -seen / norm;
        load.0 += r.lin * impulse;
        load.1 += r.torque * impulse;
    }
    load
}

fn solved_rows(joint: &ImpulseJoint, b1: &RigidBody, b2: &RigidBody) -> (Vec<Row>, Metric) {
    let mut rows = rows(joint, b1, b2);
    let metric = Metric::new(inv_mass(b1, b2), inv_mass(b2, b1));
    orthogonalize(&mut rows, &metric);
    (rows, metric)
}

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

fn rows(joint: &ImpulseJoint, b1: &RigidBody, b2: &RigidBody) -> Vec<Row> {
    let data = &joint.data;
    let f1 = *b1.position() * data.local_frame1;
    let f2 = *b2.position() * data.local_frame2;
    let basis = Mat3::from_quat(f1.rotation);
    let locked = data.locked_axes.bits();
    let limited = data.limit_axes.bits() & !locked;
    let set = |mask: u8, i: usize| mask & (1 << i) != 0;
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
        torque: Vec3::ZERO,
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

fn ang_basis(q1: Quat, q2: Quat) -> Mat3 {
    let (v1, v2, w1, w2) = (q1.xyz(), q2.xyz(), q1.w, q2.w);
    let outer = Mat3::from_cols(v1 * v2.x, v1 * v2.y, v1 * v2.z);
    let diff = (outer + Mat3::from_diagonal(Vec3::splat(w1 * w2)) - cross_mat(v1 * w2 + v2 * w1)
        + cross_mat(v1) * cross_mat(v2))
        * 0.5;
    diff.transpose() * 1f32.copysign(q1.dot(q2))
}

fn cross_mat(v: Vec3) -> Mat3 {
    Mat3::from_cols(
        Vec3::new(0.0, v.z, -v.y),
        Vec3::new(-v.z, 0.0, v.x),
        Vec3::new(v.y, -v.x, 0.0),
    )
}

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

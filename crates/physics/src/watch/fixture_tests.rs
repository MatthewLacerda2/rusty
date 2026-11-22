use std::fmt::Write;

use glam::Vec3;
use rapier3d::prelude::InteractionGroups;

use super::ops_tests::{apply, Op, Phys};
use crate::PhysicsWorld;
use rusty_ecs::changes::ops;
use rusty_scene::Scene;

pub fn fixture() -> Scene {
    let mut scene = Scene::new();
    let mut physics = PhysicsWorld::from_scene(&scene);
    for i in 0..5 {
        scene.add_entity(format!("e{i}"));
    }
    let ids = scene.world.ids().to_vec();
    let ops = [
        (0, Phys::Collider(2)),
        (1, Phys::Collider(0)),
        (1, Phys::Body(false)),
        (2, Phys::Collider(1)),
        (2, Phys::Body(false)),
        (3, Phys::Collider(0)),
        (3, Phys::Body(false)),
        (4, Phys::Collider(0)),
        (4, Phys::Body(true)),
    ];
    for (i, p) in ops {
        apply(&mut scene, &mut physics, &Op::On(i, p));
    }
    scene.world.set_static(ids[0], true);
    for (i, &id) in ids.iter().enumerate().skip(1) {
        let at = Vec3::new(-2.75 + i as f32 * 1.1, 1.0 + i as f32, 0.0);
        scene.world.transform_mut(id).unwrap().position = at;
    }
    scene
}

pub fn scene_digest(scene: &Scene) -> String {
    let w = &scene.world;
    let rows = w.ids().iter().map(|&id| {
        let rb = w.rigidbody(id).map(|r| (r.velocity, r.angular_velocity));
        let t = w.transform(id).map(|t| (t.position, t.rotation, t.scale));
        format!("{id} {t:?} {rb:?}\n")
    });
    rows.collect()
}

pub fn world_op(w: &mut rusty_ecs::World, op: &ops::Op) {
    use ops::{Edit, Op};
    let undone = match op {
        Op::On(i, Edit::UndoFlip | Edit::UndoDespawn) => w.ids().get(*i).copied(),
        _ => None,
    };
    ops::apply(w, op);
    if let Some(e) = undone {
        if let Some(p) = w.parent_id(e).filter(|&p| !w.children(p).contains(&e)) {
            w.add_child(p, e);
        }
        w.rewind_changes();
    }
}

pub fn digest(p: &PhysicsWorld) -> String {
    let mut out = format!("plan {:?}\n", p.plan);
    let mut bodies: Vec<_> = p.id_to_body.iter().collect();
    bodies.sort_by_key(|&(&id, _)| id);
    for (id, &h) in bodies {
        let b = &p.bodies[h];
        let (ty, on, asleep) = (b.body_type(), b.is_enabled(), b.is_sleeping());
        let pose = (b.position(), b.next_position());
        let (v, w, g, ccd) = (
            b.linvel(),
            b.angvel(),
            b.gravity_scale(),
            b.is_ccd_enabled(),
        );
        let _ = writeln!(
            out,
            "b{id} {ty:?} {on} {asleep} {pose:?} {v:?} {w:?} {g} {ccd} {}",
            b.mass()
        );
    }
    let mut colliders: Vec<_> = p.id_to_collider.iter().collect();
    colliders.sort_by_key(|&(&id, _)| id);
    for (id, &h) in colliders {
        let c = &p.colliders[h];
        let (on, sensor) = (c.is_enabled(), c.is_sensor());
        let bits = |g: InteractionGroups| (g.memberships.bits(), g.filter.bits());
        let groups = (bits(c.collision_groups()), bits(c.solver_groups()));
        let local = (c.position_wrt_parent(), c.shape().compute_local_aabb());
        let rules = (c.friction_combine_rule(), c.restitution_combine_rule());
        let material = (c.friction(), c.restitution(), rules);
        let _ = writeln!(
            out,
            "c{id} {on} {sensor} {groups:?} {local:?} {material:?} {}",
            c.mass()
        );
    }
    for (id, j) in &p.joints {
        let frames = p.impulse_joints.get(j.handle).map(|r| {
            let d = &r.data;
            (r.body1(), r.body2(), d.local_frame1, d.local_frame2)
        });
        let _ = writeln!(out, "j{id} {:?} {frames:?}", j.key);
    }
    // The order the solver meets the contacts in: two twins listing the same
    // pairs in another order step apart.
    let id = |h| p.collider_to_id.get(&h).copied();
    let pairs = p.narrow_phase.contact_pairs();
    let order: Vec<_> = pairs.map(|c| (id(c.collider1), id(c.collider2))).collect();
    let _ = writeln!(out, "contacts {order:?}");
    out
}

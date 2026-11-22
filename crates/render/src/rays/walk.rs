//! The CPU twin of `rt/trace.wgsl`, walking the same [`Packed`] arrays step for step.

use glam::{Vec3, Vec4};

use super::build::{Node, STACK};
use super::top::{face, Instance, Packed, NONE};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Hit {
    pub t: f32,
    pub slot: u32,
    pub tri: u32,
    pub u: f32,
    pub v: f32,
}

struct Ray {
    o: Vec3,
    d: Vec3,
    inv: Vec3,
}

impl Ray {
    fn new(o: Vec3, d: Vec3) -> Self {
        let safe = Vec3::select(d.abs().cmplt(Vec3::splat(1e-30)), Vec3::splat(1e-30), d);
        Self {
            o,
            d,
            inv: safe.recip(),
        }
    }

    fn enters(&self, n: &Node, t_max: f32) -> f32 {
        let t1 = (Vec3::from_array(n.lo) - self.o) * self.inv;
        let t2 = (Vec3::from_array(n.hi) - self.o) * self.inv;
        let near = t1.min(t2).max_element().max(0.0);
        let far = t1.max(t2).min_element().min(t_max);
        if near <= far {
            near
        } else {
            f32::INFINITY
        }
    }
}

/// The closest hit before `t_max`, or `None`.
pub(crate) fn trace_closest(p: &Packed, o: Vec3, d: Vec3, t_max: f32) -> Option<Hit> {
    walk(p, o, d, t_max, false)
}

pub(crate) fn trace_any(p: &Packed, o: Vec3, d: Vec3, t_max: f32) -> bool {
    walk(p, o, d, t_max, true).is_some()
}

fn walk(p: &Packed, o: Vec3, d: Vec3, t_max: f32, any: bool) -> Option<Hit> {
    if p.header.top == NONE {
        return None;
    }
    let ray = Ray::new(o, d);
    let mut best: Option<Hit> = None;
    let mut reach = t_max;
    visit(p, &ray, p.header.top, &mut reach, |i, reach| {
        let inst = &p.instances[i as usize];
        let found = mesh(p, inst, (o, d), reach, any);
        if let Some(h) = found {
            best = Some(h);
        }
        any && found.is_some()
    });
    best
}

fn mesh(
    p: &Packed,
    inst: &Instance,
    (o, d): (Vec3, Vec3),
    reach: &mut f32,
    any: bool,
) -> Option<Hit> {
    let rows = inst.to_local.map(Vec4::from_array);
    let lo = Vec3::new(
        rows[0].dot(o.extend(1.0)),
        rows[1].dot(o.extend(1.0)),
        rows[2].dot(o.extend(1.0)),
    );
    let ld = Vec3::new(
        rows[0].dot(d.extend(0.0)),
        rows[1].dot(d.extend(0.0)),
        rows[2].dot(d.extend(0.0)),
    );
    let ray = Ray::new(lo, ld);
    let mut best = None;
    visit(p, &ray, inst.root, reach, |tri, reach| {
        let t = &p.tris[tri as usize];
        let code = match t.run < inst.runs {
            true => p.faces[(inst.faces + t.run) as usize],
            false => face::SKIP,
        };
        let [a, b, c] = t.corners();
        let Some((t, u, v)) = triangle(&ray, (a, b, c), code, *reach) else {
            return false;
        };
        *reach = t;
        best = Some(Hit {
            t,
            slot: inst.slot,
            tri,
            u,
            v,
        });
        any
    });
    best
}

/// Walks the tree from `root` closest-first, handing each leaf item to `item`, which
/// may lower `reach` and returns true to stop.
fn visit(
    p: &Packed,
    ray: &Ray,
    root: u32,
    reach: &mut f32,
    mut item: impl FnMut(u32, &mut f32) -> bool,
) {
    let mut stack = [(0u32, 0f32); STACK];
    let entry = ray.enters(&p.nodes[root as usize], *reach);
    if entry == f32::INFINITY {
        return;
    }
    stack[0] = (root, entry);
    let mut sp = 1;
    while sp > 0 {
        sp -= 1;
        let (at, entry) = stack[sp];
        if entry > *reach {
            continue;
        }
        let n = &p.nodes[at as usize];
        if n.count > 0 {
            for i in n.first..n.first + n.count {
                if item(i, reach) {
                    return;
                }
            }
            continue;
        }
        let (l, r) = (n.first, n.first + 1);
        let tl = ray.enters(&p.nodes[l as usize], *reach);
        let tr = ray.enters(&p.nodes[r as usize], *reach);
        let (near, far) = if tl <= tr {
            ((l, tl), (r, tr))
        } else {
            ((r, tr), (l, tl))
        };
        for (node, t) in [far, near] {
            if t != f32::INFINITY && sp < STACK {
                stack[sp] = (node, t);
                sp += 1;
            }
        }
    }
}

fn triangle(
    ray: &Ray,
    (a, b, c): (Vec3, Vec3, Vec3),
    code: u32,
    reach: f32,
) -> Option<(f32, f32, f32)> {
    let (e1, e2) = (b - a, c - a);
    let pv = ray.d.cross(e2);
    let det = e1.dot(pv);
    let need = if det > 0.0 { face::FRONT } else { face::BACK };
    if det == 0.0 || code & need == 0 {
        return None;
    }
    let inv = 1.0 / det;
    let s = ray.o - a;
    let u = s.dot(pv) * inv;
    let q = s.cross(e1);
    let v = ray.d.dot(q) * inv;
    let t = e2.dot(q) * inv;
    let inside = u >= 0.0 && v >= 0.0 && u + v <= 1.0;
    (inside && t > 0.0 && t < reach).then_some((t, u, v))
}

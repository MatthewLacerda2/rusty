//! The CPU twin of `rt/trace.wgsl`, walking the same [`Packed`] arrays step for step.

use glam::{Vec3, Vec4};

use super::build::{Node, STACK};
use super::top::{face, Instance, Packed, MORE, NONE};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Hit {
    pub t: f32,
    pub slot: u32,
    pub tri: u32,
    pub u: f32,
    pub v: f32,
}

/// What one walk did: the instances it walked into, the nodes it popped (both levels)
/// and the triangles it tested.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Counts {
    pub instances: u64,
    pub nodes: u64,
    pub tris: u64,
}

#[derive(Clone, Copy)]
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
#[cfg(test)]
pub(crate) fn trace_closest(p: &Packed, o: Vec3, d: Vec3, t_max: f32) -> Option<Hit> {
    walk(p, (o, d, t_max), false, &mut Counts::default())
}

#[cfg(test)]
pub(crate) fn trace_any(p: &Packed, o: Vec3, d: Vec3, t_max: f32) -> bool {
    walk(p, (o, d, t_max), true, &mut Counts::default()).is_some()
}

/// `trace_closest`, adding what it did to `counts`.
pub(crate) fn counted(p: &Packed, ray: (Vec3, Vec3, f32), counts: &mut Counts) -> Option<Hit> {
    walk(p, ray, false, counts)
}

/// A stack entry naming an instance to enter, as `rt/trace.wgsl`'s `RT_ENTER`.
const ENTER: u32 = 1 << 31;
const MISS: f32 = f32::INFINITY;

fn local(inst: &Instance, o: Vec3, d: Vec3) -> Ray {
    let rows = inst.to_local.map(Vec4::from_array);
    let at = |v: Vec4| Vec3::new(rows[0].dot(v), rows[1].dot(v), rows[2].dot(v));
    Ray::new(at(o.extend(1.0)), at(d.extend(0.0)))
}

/// Both levels in one loop over one stack, as `rt_walk`: an instance's entries sit above
/// `base`, and popping down to it returns the ray to world space.
fn walk(p: &Packed, (o, d, t_max): (Vec3, Vec3, f32), any: bool, c: &mut Counts) -> Option<Hit> {
    if p.header.top == NONE {
        return None;
    }
    let world = Ray::new(o, d);
    let mut ray = world;
    let mut n = p.nodes[p.header.top as usize];
    if ray.enters(&n, t_max) == MISS {
        return None;
    }
    let (mut reach, mut best) = (t_max, None);
    let mut stack = [0u32; STACK];
    let (mut sp, mut base, mut pop) = (0, 0, false);
    let mut inst: Option<Instance> = None;
    loop {
        if pop {
            if inst.is_some() && sp == base {
                (inst, ray) = (None, world);
            }
            if sp == 0 {
                return best;
            }
            sp -= 1;
            let mut e = stack[sp];
            if e & ENTER != 0 {
                let k = e & !ENTER;
                let at = p.instances[k as usize];
                c.instances += 1;
                if at.runs & MORE != 0 && sp < STACK {
                    stack[sp] = (k + 1) | ENTER;
                    sp += 1;
                }
                (base, ray, e, inst) = (sp, local(&at, o, d), at.root, Some(at));
            }
            n = p.nodes[e as usize];
            if ray.enters(&n, reach) == MISS {
                continue;
            }
        }
        pop = true;
        c.nodes += 1;
        if n.count == 0 {
            let (l, r) = (p.nodes[n.first as usize], p.nodes[n.first as usize + 1]);
            let (tl, tr) = (ray.enters(&l, reach), ray.enters(&r, reach));
            let left_near = tl <= tr;
            if tl.max(tr) != MISS && sp < STACK {
                stack[sp] = if left_near { n.first + 1 } else { n.first };
                sp += 1;
            }
            if tl.min(tr) != MISS {
                (pop, n) = (false, if left_near { l } else { r });
            }
        } else if let Some(at) = &inst {
            for k in n.first..n.first + n.count {
                if let Some(h) = leaf_tri(p, at, &ray, (k, reach), c) {
                    (reach, best) = (h.t, Some(h));
                    if any {
                        return best;
                    }
                }
            }
        } else if sp < STACK {
            stack[sp] = n.first | ENTER;
            sp += 1;
        }
    }
}

fn leaf_tri(
    p: &Packed,
    inst: &Instance,
    ray: &Ray,
    (k, reach): (u32, f32),
    c: &mut Counts,
) -> Option<Hit> {
    let t = &p.tris[k as usize];
    c.tris += 1;
    let code = match t.run < (inst.runs & !MORE) {
        true => p.faces[(inst.faces + t.run) as usize],
        false => face::SKIP,
    };
    let (t, u, v) = triangle(ray, t.corners().into(), code, reach)?;
    Some(Hit {
        t,
        slot: inst.slot,
        tri: k,
        u,
        v,
    })
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

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

/// The instances one walk entered, nodes it visited and triangles it tested.
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

/// `rt/trace.wgsl`'s `RT_ENTER`.
const ENTER: u32 = 1 << 31;
const MISS: f32 = f32::INFINITY;

fn local(inst: &Instance, o: Vec3, d: Vec3) -> Ray {
    let rows = inst.to_local.map(Vec4::from_array);
    let at = |v: Vec4| Vec3::new(rows[0].dot(v), rows[1].dot(v), rows[2].dot(v));
    Ray::new(at(o.extend(1.0)), at(d.extend(0.0)))
}

/// An instance's stack entries sit above `base`.
struct Walk {
    world: Ray,
    ray: Ray,
    reach: f32,
    stack: [u32; STACK],
    sp: usize,
    base: usize,
    inst: Option<Instance>,
}

impl Walk {
    fn push(&mut self, entry: u32) {
        if self.sp < STACK {
            self.stack[self.sp] = entry;
            self.sp += 1;
        }
    }

    /// The near child the ray enters; the far one is stacked.
    fn children(&mut self, p: &Packed, n: &Node) -> Option<Node> {
        let (l, r) = (p.nodes[n.first as usize], p.nodes[n.first as usize + 1]);
        let (tl, tr) = (
            self.ray.enters(&l, self.reach),
            self.ray.enters(&r, self.reach),
        );
        let left_near = tl <= tr;
        if tl.max(tr) != MISS {
            self.push(if left_near { n.first + 1 } else { n.first });
        }
        (tl.min(tr) != MISS).then_some(if left_near { l } else { r })
    }

    /// The next stacked node the ray enters; `None` once the stack is empty.
    fn pop(&mut self, p: &Packed, (o, d): (Vec3, Vec3), c: &mut Counts) -> Option<Node> {
        loop {
            if self.inst.is_some() && self.sp == self.base {
                (self.inst, self.ray) = (None, self.world);
            }
            self.sp = self.sp.checked_sub(1)?;
            let mut e = self.stack[self.sp];
            if e & ENTER != 0 {
                let k = e & !ENTER;
                let at = p.instances[k as usize];
                c.instances += 1;
                if at.runs & MORE != 0 {
                    self.push((k + 1) | ENTER);
                }
                (self.base, self.ray, e) = (self.sp, local(&at, o, d), at.root);
                self.inst = Some(at);
            }
            let n = p.nodes[e as usize];
            if self.ray.enters(&n, self.reach) != MISS {
                return Some(n);
            }
        }
    }
}

/// Both levels in one loop over one stack, as `rt_walk`.
fn walk(p: &Packed, (o, d, t_max): (Vec3, Vec3, f32), any: bool, c: &mut Counts) -> Option<Hit> {
    if p.header.top == NONE {
        return None;
    }
    let world = Ray::new(o, d);
    let mut n = p.nodes[p.header.top as usize];
    if world.enters(&n, t_max) == MISS {
        return None;
    }
    let mut w = Walk {
        world,
        ray: world,
        reach: t_max,
        stack: [0; STACK],
        sp: 0,
        base: 0,
        inst: None,
    };
    let mut best = None;
    loop {
        c.nodes += 1;
        let next = match w.inst {
            _ if n.count == 0 => w.children(p, &n),
            Some(at) => {
                for k in n.first..n.first + n.count {
                    if let Some(h) = leaf_tri(p, &at, &w.ray, (k, w.reach), c) {
                        (w.reach, best) = (h.t, Some(h));
                        if any {
                            return best;
                        }
                    }
                }
                None
            }
            None => {
                w.push(n.first | ENTER);
                None
            }
        };
        match next.or_else(|| w.pop(p, (o, d), c)) {
            Some(node) => n = node,
            None => return best,
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

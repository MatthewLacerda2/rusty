#define_import_path rt::trace

// The ray-traced scene: `render::rays`' packed tree of the static geometry. The
// structs mirror `rays::build::Node`, `rays::mesh::{Tri, TriUv}` and
// `rays::top::{Instance, Header}` byte for byte; `rays/walk.rs` walks the same
// arrays the same way on the CPU, and the tests hold the two equal.

const RT_NONE: u32 = 0xffffffffu;
const RT_STACK: u32 = 64u;
const RT_FRONT: u32 = 1u;
const RT_BACK: u32 = 2u;

struct RtHeader {
    top: u32,
    instances: u32,
    pad_a: u32,
    pad_b: u32,
};

struct RtNode {
    lo: vec3<f32>,
    first: u32,
    hi: vec3<f32>,
    count: u32,
};

struct RtTri {
    a: vec3<f32>,
    run: u32,
    b: vec4<f32>,
    c: vec4<f32>,
};

// Per corner: UV0 in xy, the lightmap UV in zw.
struct RtTriUv {
    a: vec4<f32>,
    b: vec4<f32>,
    c: vec4<f32>,
};

struct RtInstance {
    r0: vec4<f32>,
    r1: vec4<f32>,
    r2: vec4<f32>,
    root: u32,
    slot: u32,
    faces: u32,
    runs: u32,
};

// A miss has `tri == RT_NONE` and `t` the ray's reach.
struct Hit {
    t: f32,
    slot: u32,
    tri: u32,
    bary: vec2<f32>,
};

@group(3) @binding(0) var<uniform> rt_header: RtHeader;
@group(3) @binding(1) var<storage, read> rt_nodes: array<RtNode>;
@group(3) @binding(2) var<storage, read> rt_tris: array<RtTri>;
@group(3) @binding(3) var<storage, read> rt_uvs: array<RtTriUv>;
@group(3) @binding(4) var<storage, read> rt_instances: array<RtInstance>;
@group(3) @binding(5) var<storage, read> rt_faces: array<u32>;

fn rt_inv(d: vec3<f32>) -> vec3<f32> {
    return 1.0 / select(d, vec3<f32>(1e-30), abs(d) < vec3<f32>(1e-30));
}

// The distance the ray enters `n` at, or a huge value for a miss.
fn rt_enters(o: vec3<f32>, inv: vec3<f32>, n: u32, reach: f32) -> f32 {
    let node = rt_nodes[n];
    let t1 = (node.lo - o) * inv;
    let t2 = (node.hi - o) * inv;
    let lo = min(t1, t2);
    let hi = max(t1, t2);
    let near = max(max(max(lo.x, lo.y), lo.z), 0.0);
    let far = min(min(min(hi.x, hi.y), hi.z), reach);
    return select(3.4e38, near, near <= far);
}

// Moller-Trumbore, culled as the run's material draws it: x is t, yz the barycentrics.
fn rt_triangle(o: vec3<f32>, d: vec3<f32>, tri: RtTri, code: u32, reach: f32) -> vec3<f32> {
    let e1 = tri.b.xyz - tri.a;
    let e2 = tri.c.xyz - tri.a;
    let p = cross(d, e2);
    let det = dot(e1, p);
    let need = select(RT_BACK, RT_FRONT, det > 0.0);
    if det == 0.0 || (code & need) == 0u {
        return vec3<f32>(-1.0);
    }
    let inv = 1.0 / det;
    let s = o - tri.a;
    let u = dot(s, p) * inv;
    let q = cross(s, e1);
    let v = dot(d, q) * inv;
    let t = dot(e2, q) * inv;
    let inside = u >= 0.0 && v >= 0.0 && u + v <= 1.0;
    return select(vec3<f32>(-1.0), vec3<f32>(t, u, v), inside && t > 0.0 && t < reach);
}

fn rt_mesh(i: u32, wo: vec3<f32>, wd: vec3<f32>, best: Hit, any: bool) -> Hit {
    let inst = rt_instances[i];
    let o = vec3<f32>(dot(inst.r0, vec4(wo, 1.0)), dot(inst.r1, vec4(wo, 1.0)), dot(inst.r2, vec4(wo, 1.0)));
    let d = vec3<f32>(dot(inst.r0, vec4(wd, 0.0)), dot(inst.r1, vec4(wd, 0.0)), dot(inst.r2, vec4(wd, 0.0)));
    let inv = rt_inv(d);
    var hit = best;
    var nodes: array<u32, RT_STACK>;
    var ts: array<f32, RT_STACK>;
    let entry = rt_enters(o, inv, inst.root, hit.t);
    if entry > 3.0e38 {
        return hit;
    }
    nodes[0] = inst.root;
    ts[0] = entry;
    var sp = 1u;
    while sp > 0u {
        sp -= 1u;
        if ts[sp] > hit.t {
            continue;
        }
        let n = rt_nodes[nodes[sp]];
        if n.count > 0u {
            for (var k = n.first; k < n.first + n.count; k++) {
                let tri = rt_tris[k];
                let code = select(0u, rt_faces[inst.faces + tri.run], tri.run < inst.runs);
                let found = rt_triangle(o, d, tri, code, hit.t);
                if found.x > 0.0 {
                    hit = Hit(found.x, inst.slot, k, found.yz);
                    if any {
                        return hit;
                    }
                }
            }
            continue;
        }
        let tl = rt_enters(o, inv, n.first, hit.t);
        let tr = rt_enters(o, inv, n.first + 1u, hit.t);
        let left_near = tl <= tr;
        let near = select(n.first + 1u, n.first, left_near);
        let far = select(n.first, n.first + 1u, left_near);
        let t_near = min(tl, tr);
        let t_far = max(tl, tr);
        if t_far < 3.0e38 && sp < RT_STACK {
            nodes[sp] = far;
            ts[sp] = t_far;
            sp += 1u;
        }
        if t_near < 3.0e38 && sp < RT_STACK {
            nodes[sp] = near;
            ts[sp] = t_near;
            sp += 1u;
        }
    }
    return hit;
}

fn rt_walk(o: vec3<f32>, d: vec3<f32>, t_max: f32, any: bool) -> Hit {
    var hit = Hit(t_max, RT_NONE, RT_NONE, vec2<f32>(0.0));
    if rt_header.top == RT_NONE {
        return hit;
    }
    let inv = rt_inv(d);
    var nodes: array<u32, RT_STACK>;
    var ts: array<f32, RT_STACK>;
    let entry = rt_enters(o, inv, rt_header.top, hit.t);
    if entry > 3.0e38 {
        return hit;
    }
    nodes[0] = rt_header.top;
    ts[0] = entry;
    var sp = 1u;
    while sp > 0u {
        sp -= 1u;
        if ts[sp] > hit.t {
            continue;
        }
        let n = rt_nodes[nodes[sp]];
        if n.count > 0u {
            for (var k = n.first; k < n.first + n.count; k++) {
                hit = rt_mesh(k, o, d, hit, any);
                if any && hit.tri != RT_NONE {
                    return hit;
                }
            }
            continue;
        }
        let tl = rt_enters(o, inv, n.first, hit.t);
        let tr = rt_enters(o, inv, n.first + 1u, hit.t);
        let left_near = tl <= tr;
        let near = select(n.first + 1u, n.first, left_near);
        let far = select(n.first, n.first + 1u, left_near);
        if max(tl, tr) < 3.0e38 && sp < RT_STACK {
            nodes[sp] = far;
            ts[sp] = max(tl, tr);
            sp += 1u;
        }
        if min(tl, tr) < 3.0e38 && sp < RT_STACK {
            nodes[sp] = near;
            ts[sp] = min(tl, tr);
            sp += 1u;
        }
    }
    return hit;
}

// The closest static surface along `dir` before `t_max` (world units of `dir`'s length).
fn trace_closest(origin: vec3<f32>, dir: vec3<f32>, t_max: f32) -> Hit {
    return rt_walk(origin, dir, t_max, false);
}

// Whether anything static blocks the ray before `t_max`: a shadow ray.
fn trace_any(origin: vec3<f32>, dir: vec3<f32>, t_max: f32) -> bool {
    return rt_walk(origin, dir, t_max, true).tri != RT_NONE;
}

fn hit_found(hit: Hit) -> bool {
    return hit.tri != RT_NONE;
}

// The hit's interpolated UV0 (xy) and lightmap UV (zw).
fn hit_uvs(hit: Hit) -> vec4<f32> {
    let uv = rt_uvs[hit.tri];
    let w = 1.0 - hit.bary.x - hit.bary.y;
    return uv.a * w + uv.b * hit.bary.x + uv.c * hit.bary.y;
}

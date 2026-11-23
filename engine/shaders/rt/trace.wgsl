#define_import_path rt::trace

// The ray-traced scene: `render::rays`' packed tree of the static geometry. The
// structs mirror `rays::build::Node`, `rays::mesh::{Tri, TriUv}` and
// `rays::top::{Instance, Header}` byte for byte; `rays/walk.rs` walks the same
// arrays the same way on the CPU, and the tests hold the two equal.

const RT_NONE: u32 = 0xffffffffu;
// `rays::build::STACK`: both levels' depth caps plus one, which a test holds equal.
const RT_STACK: u32 = 97u;
// A stack entry naming an instance, not a node.
const RT_ENTER: u32 = 0x80000000u;
// On `RtInstance::runs`: the next instance is in the same top leaf.
const RT_MORE: u32 = 0x80000000u;
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
    row_x: vec4<f32>,
    row_y: vec4<f32>,
    row_z: vec4<f32>,
    root: u32,
    slot: u32,
    faces: u32,
    runs: u32,
};

struct RtRay {
    o: vec3<f32>,
    d: vec3<f32>,
    inv: vec3<f32>,
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

// The distance the ray enters `node` at, or a huge value for a miss.
fn rt_enters(r: RtRay, node: RtNode, reach: f32) -> f32 {
    let t1 = (node.lo - r.o) * r.inv;
    let t2 = (node.hi - r.o) * r.inv;
    let lo = min(t1, t2);
    let hi = max(t1, t2);
    let near = max(max(max(lo.x, lo.y), lo.z), 0.0);
    let far = min(min(min(hi.x, hi.y), hi.z), reach);
    return select(3.4e38, near, near <= far);
}

fn rt_ray(o: vec3<f32>, d: vec3<f32>) -> RtRay {
    return RtRay(o, d, rt_inv(d));
}

// Moller-Trumbore, culled as the run's material draws it: x is t, yz the barycentrics.
fn rt_triangle(r: RtRay, tri: RtTri, code: u32, reach: f32) -> vec3<f32> {
    let e1 = tri.b.xyz - tri.a;
    let e2 = tri.c.xyz - tri.a;
    let p = cross(r.d, e2);
    let det = dot(e1, p);
    let need = select(RT_BACK, RT_FRONT, det > 0.0);
    if det == 0.0 || (code & need) == 0u {
        return vec3<f32>(-1.0);
    }
    let inv = 1.0 / det;
    let s = r.o - tri.a;
    let u = dot(s, p) * inv;
    let q = cross(s, e1);
    let v = dot(r.d, q) * inv;
    let t = dot(e2, q) * inv;
    let inside = u >= 0.0 && v >= 0.0 && u + v <= 1.0;
    return select(vec3<f32>(-1.0), vec3<f32>(t, u, v), inside && t > 0.0 && t < reach);
}

// The ray in instance `inst`'s mesh space; `t` stays the world ray's.
fn rt_local(inst: RtInstance, o: vec3<f32>, d: vec3<f32>) -> RtRay {
    let lo = vec3<f32>(dot(inst.row_x, vec4(o, 1.0)), dot(inst.row_y, vec4(o, 1.0)), dot(inst.row_z, vec4(o, 1.0)));
    let ld = vec3<f32>(dot(inst.row_x, vec4(d, 0.0)), dot(inst.row_y, vec4(d, 0.0)), dot(inst.row_z, vec4(d, 0.0)));
    return rt_ray(lo, ld);
}

// Both levels in one loop over one stack: an instance's entries sit above `base`, and
// popping down to it returns the ray to world space. A popped node is tested again.
fn rt_walk(o: vec3<f32>, d: vec3<f32>, t_max: f32, any: bool) -> Hit {
    var hit = Hit(t_max, RT_NONE, RT_NONE, vec2<f32>(0.0));
    if rt_header.top == RT_NONE {
        return hit;
    }
    let world = rt_ray(o, d);
    var ray = world;
    var n = rt_nodes[rt_header.top];
    if rt_enters(ray, n, hit.t) > 3.0e38 {
        return hit;
    }
    var stack: array<u32, RT_STACK>;
    var sp = 0u;
    var base = 0u;
    var inst = RT_NONE;
    var at = RtInstance();
    var pop = false;
    loop {
        if pop {
            if inst != RT_NONE && sp == base {
                inst = RT_NONE;
                ray = world;
            }
            if sp == 0u {
                break;
            }
            sp -= 1u;
            var e = stack[sp];
            if (e & RT_ENTER) != 0u {
                inst = e & ~RT_ENTER;
                at = rt_instances[inst];
                if (at.runs & RT_MORE) != 0u && sp < RT_STACK {
                    stack[sp] = (inst + 1u) | RT_ENTER;
                    sp += 1u;
                }
                base = sp;
                ray = rt_local(at, o, d);
                e = at.root;
            }
            n = rt_nodes[e];
            if rt_enters(ray, n, hit.t) > 3.0e38 {
                continue;
            }
        }
        pop = true;
        if n.count == 0u {
            let l = rt_nodes[n.first];
            let r = rt_nodes[n.first + 1u];
            let tl = rt_enters(ray, l, hit.t);
            let tr = rt_enters(ray, r, hit.t);
            let left_near = tl <= tr;
            if max(tl, tr) < 3.0e38 && sp < RT_STACK {
                stack[sp] = select(n.first, n.first + 1u, left_near);
                sp += 1u;
            }
            if min(tl, tr) < 3.0e38 {
                pop = false;
                if left_near {
                    n = l;
                } else {
                    n = r;
                }
            }
        } else if inst == RT_NONE {
            if sp < RT_STACK {
                stack[sp] = n.first | RT_ENTER;
                sp += 1u;
            }
        } else {
            let runs = at.runs & ~RT_MORE;
            for (var k = n.first; k < n.first + n.count; k++) {
                let tri = rt_tris[k];
                let code = select(0u, rt_faces[at.faces + tri.run], tri.run < runs);
                let found = rt_triangle(ray, tri, code, hit.t);
                if found.x > 0.0 {
                    hit = Hit(found.x, at.slot, k, found.yz);
                    if any {
                        return hit;
                    }
                }
            }
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

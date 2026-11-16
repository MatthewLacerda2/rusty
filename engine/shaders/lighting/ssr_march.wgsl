// engine/shaders/lighting/ssr_march.wgsl — the screen-space reflection march (#1521),
// shared by the SSR pass over the opaque image (`postfx/ssr.wgsl`) and a transparent
// surface that traces its own (`forward/transparent.wgsl`'s `reflect_ssr`, #1570).
// One march, two depth sources: the SSR pass reads each pixel's depth along the view
// axis from the velocity target's `.z`; a transparent reads the eye distance in the
// opaque copy's `.a` (`opaque_copy.wgsl`). A `Frame` says which. The SSR pass also
// hands it a min-depth pyramid of its depth (`postfx/ssr_hiz.wgsl`, #2228), so no
// step passes over a surface thinner than itself (`clear`); a transparent has none.
//
// No derivatives here (crates/render/src/CLAUDE.md, *Shaders*): every read is a
// `textureLoad` or an explicit-LOD sample, so a caller may march in a per-pixel branch.

#define_import_path lighting::ssr_march

// How far from a screen edge, as a fraction of the screen, a hit fades out (HDRP's
// *Screen Edge Fade Distance*).
const SCREEN_FADE: f32 = 0.1;
// Bisection steps that refine a hit between the last two march points.
const REFINE: i32 = 4;
// A depth at or beyond this is the sky: the velocity target's
// `render::motion::SKY_DISTANCE`, the opaque copy's `SKY_DISTANCE` (65504).
const SKY_DEPTH: f32 = 60000.0;

// What a march reads the scene against: the camera's view-projection and eye, and
// how a point's depth is measured. `fwd` is the view axis, a depth along it (the
// velocity target), or zero for the distance from the eye (the opaque copy);
// `channel` picks the depth texel's lane that holds it.
struct Frame {
    view_proj: mat4x4<f32>,
    eye: vec3<f32>,
    fwd: vec3<f32>,
    channel: vec4<f32>,
};

fn texel(t: texture_2d<f32>, uv: vec2<f32>) -> vec4<f32> {
    let dim = vec2<f32>(textureDimensions(t));
    let coord = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(0.999999)) * dim);
    return textureLoad(t, coord, 0);
}

fn depth_of(f: Frame, p: vec3<f32>) -> f32 {
    if (all(f.fwd == vec3<f32>(0.0))) {
        return distance(p, f.eye);
    }
    return dot(f.fwd, p - f.eye);
}

// Where a march point `p` lies against the screen: `on_screen` false when it left
// the screen or went behind the camera; else its `uv`, and `behind`, how far it is
// behind the surface there (> 0: behind), which counts only where `valid` (not the
// sky).
struct Probe {
    behind: f32,
    uv: vec2<f32>,
    valid: bool,
    on_screen: bool,
};

fn probe(f: Frame, t_depth: texture_2d<f32>, p: vec3<f32>) -> Probe {
    let clip = f.view_proj * vec4<f32>(p, 1.0);
    let s = clip.xy / max(clip.w, 1e-6);
    let uv = vec2<f32>(s.x * 0.5 + 0.5, 0.5 - s.y * 0.5);
    let on_screen = clip.w > 0.0 && all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
    let scene = dot(texel(t_depth, uv), f.channel);
    let valid = on_screen && scene < SKY_DEPTH;
    return Probe(depth_of(f, p) - scene, uv, valid, on_screen);
}

// How much of the march's reach, at its far end, a hit fades out over: a ray that
// hits just before running out and its neighbour that misses meet without a seam.
const REACH_FADE: f32 = 0.2;

// The on-screen diameter, in texels of `dim`, of the cone of tangent `cone` around
// the ray `r` where it hits `p`, `t` world units from where it left: the disk of
// radius `cone × t` across the ray, measured along the one direction across both the
// ray and the line of sight, which faces the camera.
fn footprint(f: Frame, p: vec3<f32>, r: vec3<f32>, t: f32, cone: f32, uv: vec2<f32>, dim: vec2<f32>) -> f32 {
    var across = cross(r, p - f.eye);
    if (dot(across, across) < 1e-10) {
        across = cross(r, select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(r.y) > 0.9));
    }
    let rim = f.view_proj * vec4<f32>(p + normalize(across) * (cone * t), 1.0);
    let s = rim.xy / max(rim.w, 1e-6);
    let rim_uv = vec2<f32>(s.x * 0.5 + 0.5, 0.5 - s.y * 0.5);
    return 2.0 * length((rim_uv - uv) * dim);
}

// The longest a march step may move on screen, as a fraction of the screen's
// shorter side (#2004): a step that would move further is shortened to it. A fixed
// world step near the camera jumps tens of texels between depth taps and steps past
// a thin object (a bottle, a pole) or a grazing floor, drawing its reflection as
// stair-steps and dashes; a step held to this sees anything wider than it, and
// with the min-depth pyramid (`clear`, #2228) anything at all, so there a step may
// span `SPAN` of these: the cap is then the unit the reach counts in. The reach so
// spent is the march's, not the world step's: `trace` fades by the steps.
// A march of fewer than 64 steps is capped at `1 / steps` of that side instead
// (`step_cap`), so any step count can still cross the screen's height: the cap
// never cuts a quality level's reach to a fraction of the screen, it only spends
// more of a long count's steps where the ray moves fast.
const SCREEN_STEP: f32 = 1.0 / 64.0;

// The cap in texels of `dim` for a march of `steps` steps: `SCREEN_STEP` or
// `1 / steps` of the shorter side, whichever is longer.
fn step_cap(steps: i32, dim: vec2<f32>) -> f32 {
    return max(SCREEN_STEP, 1.0 / f32(max(steps, 1))) * min(dim.x, dim.y);
}

// How far behind the scene's depth a refined hit may lie and still count (#2004):
// a ray that crosses a surface refines to within a texel of it, one that passes
// behind an object refines onto the object's edge, still behind it by however far
// it passed. The depth target holds only front faces, so a ray that meets an
// object's unseen back is behind its front by the object's own depth: `THICKNESS`
// is how deep an object is taken to be (a bottle, a pole, a crate's edge), and a ray
// further behind passes it; a narrow one is taken thinner (`deep`). `DEPTH_SLACK`, a fraction of the depth, is a texel's
// own depth on a surface seen near grazing: 2% holds a floor down to a few degrees
// at 1080p (a texel's depth there is about `depth × 0.001 / tan(angle)`).
const THICKNESS: f32 = 0.15;
const DEPTH_SLACK: f32 = 0.02;

// Where a hit's blur, the cone's on-screen radius as a fraction of the screen's
// shorter side, is too wide for screen space (#2004): where a ray stops hitting (a
// reflected silhouette) is decided per pixel, and only the SSR pass's resolve
// (#2028) averages it over the blur, so a wide blur drawn inside a hard edge reads
// as a block here, and the resolve's average over so wide a disk only approximates
// the lobe. From `BLUR_FADE_START` the hit fades out to the probe it replaces, gone
// at `BLUR_FADE_END`. A contact reflection (a short ray) keeps a narrow blur and
// stays; a long ray from a rough surface takes the probe.
const BLUR_FADE_START: f32 = 0.04;
const BLUR_FADE_END: f32 = 0.12;

// Where `p` lands on screen (`uv`) and whether in front of the camera: `probe`
// without the depth tap.
fn project(f: Frame, p: vec3<f32>) -> vec3<f32> {
    let clip = f.view_proj * vec4<f32>(p, 1.0);
    let s = clip.xy / max(clip.w, 1e-6);
    return vec3<f32>(s.x * 0.5 + 0.5, 0.5 - s.y * 0.5, clip.w);
}

// How far behind the scene at `p` a point may lie and still be on it.
fn slack(f: Frame, p: vec3<f32>) -> f32 {
    return max(THICKNESS, depth_of(f, p) * DEPTH_SLACK);
}

// Where a march point stands against the scene: in front of it (or over the sky),
// on it (behind by at most `slack`: a hit), or beyond it.
const FRONT: i32 = 0;
const ON: i32 = 1;
const BEYOND: i32 = 2;

fn state(f: Frame, here: Probe, p: vec3<f32>) -> i32 {
    if (!here.valid || here.behind <= 0.0) {
        return FRONT;
    }
    return select(BEYOND, ON, here.behind <= slack(f, p));
}

// Where a refinement settled: `go`, the point on the far side of the boundary,
// and `keep`, the last on the near side. The boundary lies between them, so a hit
// at `go` is placed only to within the ray's depth between the two.
struct Bracket {
    keep: vec3<f32>,
    go: vec3<f32>,
};

// Where between `a` (of state `start`) and `b` the ray first changes state, after
// `REFINE` halvings. From in front, the boundary is where it goes behind the
// scene; from beyond, where it comes back within `slack` (or in front).
fn refine(f: Frame, t_depth: texture_2d<f32>, a: vec3<f32>, b: vec3<f32>, start: i32) -> Bracket {
    var keep = a;
    var go = b;
    for (var j = 0; j < REFINE; j = j + 1) {
        let mid = (keep + go) * 0.5;
        let s = state(f, probe(f, t_depth, mid), mid);
        if (s == start) {
            keep = mid;
        } else {
            go = mid;
        }
    }
    return Bracket(keep, go);
}

// How deep a narrow object is taken to be, as a fraction of its depth (#2228):
// HDRP's default *Object Thickness*, 1%.
const THIN_SLACK: f32 = 0.01;
// How wide on screen, as a fraction of its depth, an object is at most to count as
// narrow: twice `THIN_SLACK`, so a strip whose height in texels moves by one
// between TAA's offsets stays narrow on every offset.
const THIN_WIDE: f32 = 0.02;
// The most texels `deep` looks at each side of a hit (a 4K view; 1080p needs 19).
const EXTENT_TAPS: i32 = 24;
// What `extent` returns for an object that runs on past its taps.
const OPEN: f32 = 1.0e9;

// How many texels, through `uv` along `axis` (texels, unit), the object seen there
// spans: the texel and those each side of it whose depths run on within `tol` of
// each other; `OPEN` when it runs on for all `taps` on either side, so a hit on a
// wide object's edge, whose outer side reads the background, still finds it wide.
fn extent(f: Frame, t_depth: texture_2d<f32>, uv: vec2<f32>, axis: vec2<f32>, tol: f32, taps: i32) -> f32 {
    let dim = vec2<f32>(textureDimensions(t_depth));
    let d0 = dot(texel(t_depth, uv), f.channel);
    var count = 1;
    for (var side = -1.0; side <= 1.0; side = side + 2.0) {
        var last = d0;
        var k = 1;
        for (; k <= taps; k = k + 1) {
            let q = uv + axis * (side * f32(k)) / dim;
            if (any(q < vec2<f32>(0.0)) || any(q > vec2<f32>(1.0))) {
                break;
            }
            let d = dot(texel(t_depth, q), f.channel);
            if (abs(d - last) > tol) {
                break;
            }
            last = d;
        }
        if (k > taps) {
            return OPEN;
        }
        count = count + k - 1;
    }
    return f32(count);
}

// The world size of one texel of `dim` across the line of sight at `p`.
fn texel_size(f: Frame, p: vec3<f32>, dim: vec2<f32>) -> f32 {
    let sight = p - f.eye;
    var across = cross(sight, vec3<f32>(0.0, 1.0, 0.0));
    if (dot(across, across) < 1e-10) {
        across = cross(sight, vec3<f32>(1.0, 0.0, 0.0));
    }
    let moved = length((project(f, p + normalize(across) * 0.01).xy - project(f, p).xy) * dim);
    return 0.01 / max(moved, 1e-6);
}

// How far behind the object a hit at `p` (seen at `uv`) may lie and still hit it
// (#2228): `THIN_SLACK` of its depth for an object narrower on screen than
// `THIN_WIDE` of its depth (a strip, a cable, a thin pole), `slack` for any other
// (a crate, a wall, a grazing floor, a wider pole), as before. Its width is the
// narrower of its `extent`s along the screen's rows and its columns, not along the
// ray: a vertical pole over a mirror floor runs on along its ray. The depth target
// holds only front faces, so a ray passing behind an object hits it only within
// its thickness: a fixed `THICKNESS` drew a 6 mm strip's reflection as wide as a
// 15 cm deep one's once the march stopped stepping over it. Two thicknesses, not
// one in proportion to the width: a width in texels moves by one between TAA's
// offsets, and a thickness scaled by it moves a strip's reflected edge with it,
// the whole strip at once (a 12 mm strip's TAA dip: 12.0 at 1.5 widths deep,
// against 1.2), while a leaning 8 cm pole lost rows of its far side at one width
// deep. So what is wider than `THIN_WIDE` keeps `THICKNESS`, which it judges no
// better than `main` did.
fn deep(f: Frame, t_depth: texture_2d<f32>, p: vec3<f32>, uv: vec2<f32>) -> f32 {
    let tol = slack(f, p);
    let dim = vec2<f32>(textureDimensions(t_depth));
    let depth = depth_of(f, p);
    let size = texel_size(f, p, dim);
    let wide = THIN_WIDE * depth / size;
    let taps = clamp(i32(ceil(wide)), 1, EXTENT_TAPS);
    let rows = extent(f, t_depth, uv, vec2<f32>(1.0, 0.0), tol, taps);
    let across = min(rows, select(OPEN, extent(f, t_depth, uv, vec2<f32>(0.0, 1.0), tol, taps), rows > wide));
    return select(tol, min(depth * THIN_SLACK, tol), across <= wide);
}

// A step shorter than this many texels on screen is probed at its end alone, and
// one `clear` finds something nearer under is probed every this many: any texel
// the ray's path runs through for at least this long holds a probe.
const PROBE: f32 = 0.5;
// How many of `step_cap`'s steps one step spans where `clear` sees what it passes.
const SPAN: f32 = 4.0;
// The most a march spends in `clear` on one step, in its cell reads and probes'
// rounds: past it the step's end alone is probed, as without a pyramid.
const CLEAR_ROUNDS: i32 = 32;

// A point's position on the screen of `dim`, in texels, from its clip position `c`.
fn texels(c: vec4<f32>, dim: vec2<f32>) -> vec2<f32> {
    let s = c.xy / c.w;
    return vec2<f32>(s.x * 0.5 + 0.5, 0.5 - s.y * 0.5) * dim;
}

// The world fraction along a segment whose ends have clip `w`s `wa` and `wb` at
// screen fraction `u` along it (perspective: the screen fraction is not the world's).
fn world_fraction(u: f32, wa: f32, wb: f32) -> f32 {
    return u * wa / max((1.0 - u) * wb + u * wa, 1e-6);
}

// The nearest depth under cell `cell` of `level` (2^level texels of `dim` across)
// in the min-depth pyramid `t_hiz`, whose mip `level - 1` holds that level's
// cells; a cell past the grid reads the edge's. Read through `s` (nearest) at the
// cell's centre: a mip level varying per pixel reads the wrong level through
// `textureLoad` on lavapipe (crates/render/src/CLAUDE.md).
fn cell_min(t_hiz: texture_2d<f32>, s: sampler, level: i32, dim: vec2<f32>, cell: vec2<f32>) -> f32 {
    let cells = max(floor(dim / exp2(f32(level))), vec2<f32>(1.0));
    let c = clamp(cell, vec2<f32>(0.0), cells - 1.0);
    return textureSampleLevel(t_hiz, s, (c + 0.5) / cells, f32(level - 1)).r;
}

// Where along the step from `a` to `b`, which starts in front of the scene, the
// march must look (#2228): `.y` the world fraction of the first point that may not
// be in front (1: the step's end, as a march without a pyramid), `.x` that of the
// last point it probed in front (0: none), where a refinement starts. HDRP's Hi-Z
// traversal, within one step: the ray's screen path is walked cell by cell of the
// min-depth pyramid `t_hiz` (`levels` levels, 0: none), a cell passed in one read
// when the ray's farthest depth across it is nearer than the nearest under it, then
// the next at the level above; a cell it cannot pass is entered at the level below,
// down to 2-texel cells, whose stretch of the path is probed every `PROBE` texels.
// So a 1 px line the ray passes behind is probed wherever the ray crosses it, on
// every sub-pixel offset of TAA's jitter, where a probe at the step's end alone
// found it on some and missed it on others. The ray's depth across a cell is the
// larger at its ends, true of a depth along the view axis (`f.fwd`, the SSR
// pass), not of an eye distance (a transparent's march, which has no pyramid).
fn clear(f: Frame, t_depth: texture_2d<f32>, t_hiz: texture_2d<f32>, s: sampler, levels: i32, a: vec3<f32>, b: vec3<f32>) -> vec2<f32> {
    let ca = f.view_proj * vec4<f32>(a, 1.0);
    let cb = f.view_proj * vec4<f32>(b, 1.0);
    if (levels <= 0 || ca.w <= 0.0 || cb.w <= 0.0) {
        return vec2<f32>(0.0, 1.0);
    }
    let dim = vec2<f32>(textureDimensions(t_depth));
    let ua = texels(ca, dim);
    let path = texels(cb, dim) - ua;
    let n = length(path);
    if (n <= PROBE) {
        return vec2<f32>(0.0, 1.0);
    }
    let top = clamp(i32(ceil(log2(n))), 1, levels);
    // A hundredth of a texel: how far into a cell its walk starts.
    let inside = 0.01 / n;
    var level = top;
    var u = 0.0;
    var near_end = depth_of(f, a);
    var front = 0.0;
    for (var k = 0; k < CLEAR_ROUNDS && u < 1.0; k = k + 1) {
        let size = exp2(f32(level));
        let cell = floor((ua + path * min(u + inside, 1.0)) / size);
        // Where the path leaves the cell: its first boundary ahead on either axis.
        let edge = (cell + select(vec2<f32>(0.0), vec2<f32>(1.0), path > vec2<f32>(0.0))) * size;
        let exits = select(vec2<f32>(2.0), (edge - ua) / path, abs(path) > vec2<f32>(1e-6));
        let v = clamp(min(exits.x, exits.y), u + inside, 1.0);
        let far_end = depth_of(f, mix(a, b, world_fraction(v, ca.w, cb.w)));
        if (max(near_end, far_end) < cell_min(t_hiz, s, level, dim, cell)) {
            u = v;
            near_end = far_end;
            front = world_fraction(v, ca.w, cb.w);
            level = min(level + 1, top);
            continue;
        }
        if (level > 1) {
            level = level - 1;
            continue;
        }
        let count = i32(ceil((v - u) * n / PROBE));
        for (var j = 1; j <= count; j = j + 1) {
            let w = world_fraction(min(u + f32(j) * PROBE / n, v), ca.w, cb.w);
            let q = mix(a, b, w);
            if (state(f, probe(f, t_depth, q), q) != FRONT) {
                return vec2<f32>(front, w);
            }
            front = w;
        }
        u = v;
        near_end = far_end;
        level = min(level + 1, top);
    }
    // Out of rounds: the step ends where the walk got to, all of it seen, and the
    // march goes on from there with a step of its own.
    return vec2<f32>(front, select(world_fraction(u, ca.w, cb.w), 1.0, u >= 1.0));
}

// March the reflected ray `r` from `origin` in `steps` steps of at most `step`
// world units (and at most `step_cap` on screen), against `t_depth`'s
// depths as `f` measures them. Each point is in front of the scene, on it (behind
// by at most `slack`: an object `THICKNESS` deep) or beyond it. A step that
// leaves the front, or comes back from beyond, is refined by bisection to where it
// changed (`refine`), and hits only if that point is on the scene, no further
// behind it than its object is deep (`deep`): one that went behind a thin
// object's edge, or came out beside it, marches on. Returns the
// colour of `t_color` it hits and, in `.w`, how much the hit counts (0: a miss):
// faded at the screen's edge, wider by the blur's radius, over the march's last
// `REACH_FADE` of its steps, and out where the blur is too wide to hide a hard
// edge (`BLUR_FADE_START`).
//
// `cone` is the tangent of the half-angle of the surface's GGX lobe (0: a mirror,
// #1736): the hit is read at the mip of `t_color` whose texels are the cone's
// on-screen width there, so a rough surface reflects a blur of what it sees (cone
// tracing, as Frostbite and UE do). A `t_color` with one level reads sharp.
fn trace(f: Frame, t_depth: texture_2d<f32>, t_color: texture_2d<f32>, s_color: sampler, origin: vec3<f32>, r: vec3<f32>, steps: i32, step: f32, cone: f32) -> vec4<f32> {
    return trace_hit(f, t_depth, t_depth, s_color, 0, t_color, s_color, origin, r, steps, step, cone, 1.0).colour;
}

// What `trace_hit` found: `trace`'s colour and weight, and `dist`, how far the ray
// went from its origin to the hit (0 on a miss). The SSR pass's resolve
// (`postfx/ssr.wgsl`) widens a rough hit by it (#2028).
struct Hit {
    colour: vec4<f32>,
    dist: f32,
};

// `trace`, with the hit's distance, reading the hit's colour at `blur` times the
// cone's width (0: the pyramid's first level, for a caller that blurs the hits
// itself; the fades still count the whole cone), and stepping past nothing thinner
// than a step by `t_hiz`, a min-depth pyramid of `t_depth` of `hiz_levels` levels
// (0: none, each step probed at its end; `clear`). A step `clear` cuts short at
// something that turns out not to be a hit counts for the share of it marched, so
// the march's reach is its steps' whatever it probed on the way.
fn trace_hit(f: Frame, t_depth: texture_2d<f32>, t_hiz: texture_2d<f32>, s_hiz: sampler, hiz_levels: i32, t_color: texture_2d<f32>, s_color: sampler, origin: vec3<f32>, r: vec3<f32>, steps: i32, step: f32, cone: f32, blur_scale: f32) -> Hit {
    let depth_dim = vec2<f32>(textureDimensions(t_depth));
    let cap = step_cap(steps, depth_dim);
    var prev = origin;
    var prev_uv = project(f, origin).xy;
    var was = FRONT;
    // How many steps' worth the march has gone; the rounds are bounded too, at
    // twice the steps, for a ray cut short again and again.
    var gone = 0.0;
    for (var i = 0; i < 2 * steps && gone < f32(steps); i = i + 1) {
        // `unit`: a step as `step_cap` holds it, what the reach counts in.
        var unit = step;
        let ahead = project(f, prev + r * step);
        let moved = length((ahead.xy - prev_uv) * depth_dim);
        if (ahead.z > 0.0 && moved > cap) {
            unit = step * cap / moved;
        }
        // In front of the scene with a pyramid, `clear` sees whatever a step
        // passes over, so a step spans `SPAN` units (never past `step` nor the
        // reach left): fewer rounds for a ray fast on screen, the same reach. Not
        // the first step: the ray leaves its surface 2 cm off it, and a probe half a
        // texel out reads that surface's next texel, nearer on screen, as in front
        // of the ray, which would hit its own surface; probed at its end alone, as
        // without a pyramid, it never did.
        let scans = was == FRONT && hiz_levels > 0 && i > 0;
        let spans = select(1.0, SPAN, scans);
        let len = min(min(unit * spans, step), max(f32(steps) - gone, 0.0) * unit);
        var p = prev + r * len;
        // Where a refinement starts: the last point known to be in `was`'s state.
        var start = prev;
        if (scans) {
            let cut = clear(f, t_depth, t_hiz, s_hiz, hiz_levels, prev, p);
            start = mix(prev, p, cut.x);
            p = mix(prev, p, cut.y);
        }
        let here = probe(f, t_depth, p);
        if (!here.on_screen) {
            break;
        }
        let now = state(f, here, p);
        if (now != was) {
            let found = refine(f, t_depth, start, p, was);
            var back = found.go;
            var hit = probe(f, t_depth, back);
            // How much deeper than where the ray changed state the hit may lie,
            // from where it is placed alone: the ray's depth across the bracket.
            var placed = abs(depth_of(f, back) - depth_of(f, found.keep));
            // The halving can settle on another boundary than the one the step
            // crossed (two edges between its ends): a step that landed on the scene
            // still hits there, placed only to within the step.
            if (state(f, hit, back) != ON && now == ON) {
                back = p;
                hit = here;
                placed = abs(depth_of(f, p) - depth_of(f, start));
            }
            // On the scene, and no further behind it than its object is deep, give
            // or take where the refinement could place it: a step `clear` walked is
            // bracketed within a probe's spacing, so its hit is judged by the
            // object's depth alone; one it did not (the first, one under a texel,
            // one behind an object, every step of a march with no pyramid) may span
            // a metre, and its hit is not lost for lying deeper than the ray went.
            let lands = state(f, hit, back) == ON && hit.behind <= deep(f, t_depth, back, hit.uv) + placed;
            if (lands) {
                let dim = vec2<f32>(textureDimensions(t_color));
                let t = distance(origin, back);
                let width = footprint(f, back, r, t, cone, hit.uv, dim);
                let top = f32(textureNumLevels(t_color) - 1u);
                let lod = clamp(log2(max(width * blur_scale, 1.0)), 0.0, top);
                let edge = min(min(hit.uv.x, 1.0 - hit.uv.x), min(hit.uv.y, 1.0 - hit.uv.y));
                let blur = 0.5 * width / min(dim.x, dim.y);
                let done = gone + distance(prev, back) / unit;
                let far = clamp((f32(steps) - done) / (f32(steps) * REACH_FADE), 0.0, 1.0);
                let wide = clamp((BLUR_FADE_END - blur) / (BLUR_FADE_END - BLUR_FADE_START), 0.0, 1.0);
                // Explicit-LOD sample: a gradient instruction inside this break-bounded
                // (varying-iteration) loop fails to compile under D3D12/FXC.
                // At the centre of the depth texel the hit was tested against: a
                // refined hit lies on that texel's edge, and a filtered read there
                // would take half its colour from the surface beside it.
                let centre = (floor(clamp(hit.uv, vec2<f32>(0.0), vec2<f32>(0.999999)) * depth_dim) + 0.5) / depth_dim;
                let colour = textureSampleLevel(t_color, s_color, centre, lod).rgb;
                return Hit(vec4<f32>(colour, clamp(edge / (SCREEN_FADE + blur), 0.0, 1.0) * far * wide), t);
            }
        }
        gone = gone + distance(prev, p) / unit;
        prev = p;
        prev_uv = here.uv;
        // A change that hit nothing passed behind a thin object: the march goes on
        // behind it, as behind one too far in front to touch.
        was = select(now, BEYOND, now == ON);
    }
    return Hit(vec4<f32>(0.0), 0.0);
}

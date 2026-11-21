// engine/shaders/postfx/taa/surface.wgsl — the surfaces TAA's resolve
// (`postfx/taa.wgsl`) reprojects and tells a reveal by: each pixel's 3×3
// frontmost surface, its distance from its camera, and how far a surface is a
// reveal against last frame's.

#define_import_path postfx::taa::surface

#import numeric
#import postfx::taa::io::{taa, t_velocity, t_surface_history, MAX_CAMERAS, MAX_HDR}

// What the resolve reads of a pixel's velocity ([`surface_of`] for a surface): its
// 3×3's frontmost surface's NDC motion and that surface, and the pixel's own.
struct Front {
    motion: vec2<f32>,
    near: vec2<f32>,
    own: vec2<f32>,
};

// Whether velocity texel `a` is in front of `b`: motion blur's order (`behind` in
// `motion_blur.wgsl`). A higher camera of the stack (`.w`) is in front whatever the
// depths say, so a gun 0.6 m from its camera stays over a wall 0.35 m from the
// world camera; on one camera, the nearer view depth (`.z`) is.
fn in_front(a: vec4<f32>, b: vec4<f32>) -> bool {
    if (a.w != b.w) {
        return a.w > b.w;
    }
    return a.z < b.z;
}

// The frontmost surface in `p`'s 3×3 (URP's motion quality 2, by [`in_front`]): its
// NDC motion (none when it isn't finite) and its surface, as `p`'s ray sees it, and
// `p`'s own surface. The reveal test reads the frontmost, as FSR2's depth clip reads
// the dilated depth, so a sub-pixel gap the jitter sees the sky through in this
// frame and not the last is no reveal; the history keeps the pixel's own, so a
// sub-pixel wire the jitter misses in this frame leaves sky beside it there.
fn frontmost(p: vec2<i32>) -> Front {
    let last = vec2<i32>(textureDimensions(t_velocity)) - 1;
    let here = textureLoad(t_velocity, clamp(p, vec2<i32>(0), last), 0);
    var front = here;
    // The motion dilation: URP's motion quality 2 the 3×3, 1 its 5-tap cross, 0 none
    // (each pixel reprojects by its own motion).
    let dilation = taa.levels.y;
    if (dilation >= 1.0) {
        for (var y = -1; y <= 1; y++) {
            for (var x = -1; x <= 1; x++) {
                if (dilation < 2.0 && x != 0 && y != 0) {
                    continue;
                }
                let t = textureLoad(t_velocity, clamp(p + vec2<i32>(x, y), vec2<i32>(0), last), 0);
                if (in_front(t, front)) {
                    front = t;
                }
            }
        }
    }
    let motion = select(vec2<f32>(0.0), front.xy, numeric::finite2(front.xy));
    return Front(motion, surface_of(p, front), surface_of(p, here));
}

// Velocity texel `t` of pixel `p` as the history keeps it: the camera-to-point
// distance (from the view depth and the pixel's ray through its camera's lens), and
// the camera's index. A camera turn leaves a static point's distance alone, where
// its view depth (along the turning axis) changes: a flick is not a reveal.
fn surface_of(p: vec2<i32>, t: vec4<f32>) -> vec2<f32> {
    let lens = taa.lenses[min(u32(max(t.w, 0.0)), MAX_CAMERAS - 1u)];
    let uv = (vec2<f32>(p) + 0.5) / vec2<f32>(textureDimensions(t_velocity));
    let ndc = (uv * 2.0 - 1.0) * vec2<f32>(1.0, -1.0);
    let side = ndc * lens.xy * mix(1.0, t.z, lens.z);
    return vec2<f32>(min(length(vec3<f32>(side, t.z)), MAX_HDR), t.w);
}

// How far surface `now` is a reveal against the history's surface `then`: on one
// camera, the relative distance gap with `now` the farther (a surface last frame
// held nearer is what left); across cameras, by stack order, so a higher camera's
// surface gone (a gun bobbing off a wall) is a reveal, and one arriving (the gun
// bobbing onto it) is not, whatever the two cameras' distances.
fn reveal_gap(then: vec2<f32>, now: vec2<f32>) -> f32 {
    if (then.y != now.y) {
        return select(0.0, MAX_HDR, then.y > now.y);
    }
    return max(now.x - then.x, 0.0) / max(then.x, 1e-4);
}

// How far surface `now` is a reveal around history pixel `at` (FSR2's depth clip):
// the least [`reveal_gap`] in its 3×3, so a silhouette or a sub-pixel wire the
// jitter moved by a pixel still finds its own surface, and only a reveal does not.
// The 3×3 steps a scene pixel at a time: under TAAU that is several history pixels,
// so the wire is found at any render scale.
fn depth_mismatch(at: vec2<f32>, now: vec2<f32>) -> f32 {
    let last = vec2<i32>(textureDimensions(t_surface_history)) - 1;
    let c = vec2<i32>(floor(at));
    let step = max(taa.size.xy * taa.scene.zw, vec2<f32>(1.0));
    var best = MAX_HDR;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let k = clamp(c + vec2<i32>(round(vec2<f32>(f32(x), f32(y)) * step)), vec2<i32>(0), last);
            best = min(best, reveal_gap(textureLoad(t_surface_history, k, 0).xy, now));
        }
    }
    return best;
}

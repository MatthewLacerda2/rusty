
#define_import_path postfx::taa::surface

#import numeric
#import postfx::taa::io::{taa, t_velocity, t_surface_history, MAX_CAMERAS, MAX_HDR}

struct Front {
    motion: vec2<f32>,
    near: vec2<f32>,
    own: vec2<f32>,
};

// A higher camera of the stack (`.w`) is in front whatever the depths say, so a gun 0.6
// m from its camera stays over a wall 0.35 m from the world camera.
fn in_front(a: vec4<f32>, b: vec4<f32>) -> bool {
    if (a.w != b.w) {
        return a.w > b.w;
    }
    return a.z < b.z;
}

// The reveal test reads the frontmost, as FSR2's depth clip reads the dilated depth, so
// a sub-pixel gap the jitter sees sky through in this frame and not the last is no
// reveal; the history keeps the pixel's own.
fn frontmost(p: vec2<i32>) -> Front {
    let last = vec2<i32>(textureDimensions(t_velocity)) - 1;
    let here = textureLoad(t_velocity, clamp(p, vec2<i32>(0), last), 0);
    var front = here;
    // URP's motion quality: 2 the 3×3, 1 its 5-tap cross, 0 none.
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

// A camera turn leaves a static point's distance alone, where its view depth changes: a
// flick is not a reveal.
fn surface_of(p: vec2<i32>, t: vec4<f32>) -> vec2<f32> {
    let lens = taa.lenses[min(u32(max(t.w, 0.0)), MAX_CAMERAS - 1u)];
    let uv = (vec2<f32>(p) + 0.5) / vec2<f32>(textureDimensions(t_velocity));
    let ndc = (uv * 2.0 - 1.0) * vec2<f32>(1.0, -1.0);
    let side = ndc * lens.xy * mix(1.0, t.z, lens.z);
    return vec2<f32>(min(length(vec3<f32>(side, t.z)), MAX_HDR), t.w);
}

// Across cameras, by stack order: a higher camera's surface gone (a gun bobbing off a
// wall) is a reveal, one arriving is not.
fn reveal_gap(then: vec2<f32>, now: vec2<f32>) -> f32 {
    if (then.y != now.y) {
        return select(0.0, MAX_HDR, then.y > now.y);
    }
    return max(now.x - then.x, 0.0) / max(then.x, 1e-4);
}

// FSR2's depth clip: the least gap in its 3×3, so a sub-pixel wire the jitter moved
// still finds its own surface. Under TAAU the 3×3 spans several history pixels.
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

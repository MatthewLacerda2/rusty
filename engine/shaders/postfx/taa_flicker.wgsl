// TAA's flicker hold, what the camera's anti-flicker runs (`taa.wgsl`'s `_held` entry points),
// after HDRP's anti-flicker and Unreal TSR's flickering luminance: a still pixel whose frame
// keeps blinking under the jitter (an SSR or a specular outlier on some sub-pixel offsets)
// keeps its history's luma against the next blink instead of being clamped to it; its hue and
// saturation stay the clamp's.

#define_import_path postfx::taa_flicker

#import numeric
#import postfx::ycocg

struct Flicker {
    step: f32,
    flicker: f32,
    recurring: f32,
    // how far (0 to 1) the history is kept from its clamp toward where it was
    hold: f32,
};

const NONE: Flicker = Flicker(0.0, 0.0, 0.0, 0.0);

// How far (0 to 1) along the line from the clamped history's luma `boxed` to its own `raw`
// (linear) the history may stay: as far as `room` reaches in perceptual luma, so a bright
// history is held by what the eye sees of it, not by its linear excess.
fn reach(raw: f32, boxed: f32, room: f32) -> f32 {
    let p_raw = ycocg::perceptual(raw);
    let p_boxed = ycocg::perceptual(boxed);
    if (abs(p_raw - p_boxed) <= room) {
        return 1.0;
    }
    let p = clamp(p_boxed + sign(p_raw - p_boxed) * room, 0.0, 0.999);
    return clamp((p / (1.0 - p) - boxed) / (raw - boxed), 0.0, 1.0);
}

// The pixel's flicker this frame, from last frame's texel `then` (luma, step, flicker,
// recurring), this frame's perceptual `luma`, how `still` it is (`.x`: unmoved and unrevealed;
// `.y`: that, and not reactive), its history's luma before and after its clamp (`history`: raw,
// boxed) and the knobs (`decay`, `scale`: how many times its recurring flicker the history may
// sit outside).
fn next(then: vec4<f32>, luma: f32, still: vec2<f32>, history: vec2<f32>, knobs: vec2<f32>) -> Flicker {
    let room = knobs.y * then.w;
    let hold = reach(history.x, history.y, room) * still.y;
    let step = (luma - then.x) * still.x;
    let blink = select(0.0, min(abs(step), abs(then.y)), step * then.y < 0.0);
    let flicker = max(then.z * knobs.x, blink) * still.x;
    let recurring = max(then.w * knobs.x, min(blink, then.z)) * still.x;
    let ok = numeric::finite4(vec4<f32>(step, flicker, recurring, hold));
    let kept = select(vec3<f32>(0.0), vec3<f32>(step, flicker, recurring), ok);
    return Flicker(kept.x, kept.y, kept.z, select(0.0, hold, ok && room > 0.0));
}

fn texel(luma: f32, f: Flicker) -> vec4<f32> {
    return vec4<f32>(luma, f.step, f.flicker, f.recurring);
}

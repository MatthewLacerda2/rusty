// engine/shaders/postfx/taa_flicker.wgsl — TAA's flicker hold (#2228), what the
// camera's anti-flicker runs (`taa.wgsl`'s `_held` entry points), after HDRP's
// anti-flicker and Unreal TSR's flickering luminance: a still pixel whose frame
// keeps blinking under the jitter (an SSR or a specular outlier on some sub-pixel
// offsets) keeps its history's luma against the next blink instead of being
// clamped to it.
// The resolve's box is built from this frame alone (`taa.wgsl`), so an outlier as
// wide as its own 3×3 is its own box, and a clamp hands it to the output at nearly
// full depth.
//
// A blink is a luma step undone on the very next frame (the smaller of the two).
// A pixel's flicker is its largest blink lately, and what holds is the part of a
// blink an earlier one already reached (its recurring flicker), each keeping
// `decay` of itself a frame (`render::postfx::effects::taa::params`). So a lone
// change on a pixel that was not blinking is clamped as URP's is, both ways: a
// light switched on, a shadow or a reflection crossing a still floor (on, then
// off frames later: no blink), and a lone muzzle flash (one blink, recurring only
// as far as the pixel blinked before: a pixel already blinking holds that much of
// its end). A one-frame change repeated is held as a flicker is, from its third
// time. A moving or revealed pixel keeps no hold and forgets its flicker; a
// reactive one keeps no hold. Luma is URP's perceptual `y / (1 + y)` throughout,
// so a bright pixel's blink counts as the eye sees it.

#define_import_path postfx::taa_flicker

#import numeric
#import postfx::ycocg

// One pixel's flicker after this frame.
struct Flicker {
    // this frame's signed step, flicker and recurring flicker, for the next frame
    step: f32,
    flicker: f32,
    recurring: f32,
    // how far (0 to 1) the history is kept from its clamp toward where it was
    hold: f32,
};

// No flicker: a pixel with no history to blend.
const NONE: Flicker = Flicker(0.0, 0.0, 0.0, 0.0);

// How far (0 to 1) along the line from the clamped history's luma `boxed` to its
// own `raw` (linear) the history may stay: as far as `room` reaches in perceptual
// luma, so a bright history is held by what the eye sees of it, not by its linear
// excess.
fn reach(raw: f32, boxed: f32, room: f32) -> f32 {
    let p_raw = ycocg::perceptual(raw);
    let p_boxed = ycocg::perceptual(boxed);
    if (abs(p_raw - p_boxed) <= room) {
        return 1.0;
    }
    let p = clamp(p_boxed + sign(p_raw - p_boxed) * room, 0.0, 0.999);
    return clamp((p / (1.0 - p) - boxed) / (raw - boxed), 0.0, 1.0);
}

// The pixel's flicker this frame, from last frame's texel `then` (luma, step,
// flicker, recurring), this frame's perceptual `luma`, how `still` it is (`.x`:
// unmoved and unrevealed; `.y`: that, and not reactive), its history's luma
// before and after its clamp (`history`: raw, boxed) and the knobs (`decay`,
// `scale`: how many times its recurring flicker the history may sit outside).
fn next(then: vec4<f32>, luma: f32, still: vec2<f32>, history: vec2<f32>, knobs: vec2<f32>) -> Flicker {
    let room = knobs.y * then.w;
    let hold = reach(history.x, history.y, room) * still.y;
    let step = (luma - then.x) * still.x;
    let blink = select(0.0, min(abs(step), abs(then.y)), step * then.y < 0.0);
    let flicker = max(then.z * knobs.x, blink) * still.x;
    let recurring = max(then.w * knobs.x, min(blink, then.z)) * still.x;
    // A non-finite texel (a poisoned history) holds nothing and is forgotten.
    let ok = numeric::finite4(vec4<f32>(step, flicker, recurring, hold));
    let kept = select(vec3<f32>(0.0), vec3<f32>(step, flicker, recurring), ok);
    return Flicker(kept.x, kept.y, kept.z, select(0.0, hold, ok && room > 0.0));
}

// The flicker texel the resolve writes: this frame's `luma`, then `f`'s.
fn texel(luma: f32, f: Flicker) -> vec4<f32> {
    return vec4<f32>(luma, f.step, f.flicker, f.recurring);
}

// The SSR resolve's Vogel spiral: tap i sits at angle i × the golden angle, radius √((i + ½)/TAPS).

#define_import_path lighting::ssr_vogel

const TAPS: i32 = 12;
const GOLDEN_ANGLE_RADIANS: f32 = 2.3999632;
// Neighbouring taps sit about √(π/TAPS) of the radius apart: 0.5 holds for 12 taps, not others.
const TAP_GAP_PER_RADIUS: f32 = 0.5;

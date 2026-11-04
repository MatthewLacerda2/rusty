// engine/shaders/numeric.wgsl — whether a value is finite, for the passes that keep
// an `inf` or NaN out of what they write (motion blur's streaks, SSR's colour
// pyramid, TAA's history, #1702). WGSL has no generics: one function per width.
//
// An infinity or NaN has every exponent bit set, so the test is on the bits: a
// shader compiler may fold away a NaN compare (`v != v`).

#define_import_path numeric

const EXPONENT: u32 = 0x7f800000u;

fn finite2(v: vec2<f32>) -> bool {
    return all((bitcast<vec2<u32>>(v) & vec2<u32>(EXPONENT)) != vec2<u32>(EXPONENT));
}

fn finite3(v: vec3<f32>) -> bool {
    return all((bitcast<vec3<u32>>(v) & vec3<u32>(EXPONENT)) != vec3<u32>(EXPONENT));
}

fn finite4(v: vec4<f32>) -> bool {
    return all((bitcast<vec4<u32>>(v) & vec4<u32>(EXPONENT)) != vec4<u32>(EXPONENT));
}

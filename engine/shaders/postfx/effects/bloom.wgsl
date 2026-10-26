// engine/shaders/postfx/effects/bloom.wgsl — bloom's add, in the composite: the
// blurred bright pass (`postfx/bloom.wgsl`, run before the composite) over the HDR
// image, and the lens dirt it lights (#1442), as Unity's `UberPost` adds them:
// `bloom *= intensity; color += bloom; color += dirt * dirt_intensity * bloom`.
// Slot 0: x intensity, y intensity × dirt intensity (`0`: no dirt), w `1` while
// bloom runs.

#define_import_path postfx::effects::bloom

#import postfx::frame::{Frag, t_bloom, t_depth, t_dirt, s_color}

// `uv` on the dirt, cropped to cover the view without stretching it, centred, as
// Unity's `_LensDirt_Params` scale and offset it: the axis the dirt is longer on
// than the view is cut at both ends.
fn dirt_uv(uv: vec2<f32>) -> vec2<f32> {
    let dirt = vec2<f32>(textureDimensions(t_dirt));
    let view = vec2<f32>(textureDimensions(t_depth));
    let dirt_ratio = dirt.x / dirt.y;
    let view_ratio = view.x / view.y;
    var scale = vec2<f32>(1.0);
    if (dirt_ratio > view_ratio) {
        scale.x = view_ratio / dirt_ratio;
    } else if (view_ratio > dirt_ratio) {
        scale.y = dirt_ratio / view_ratio;
    }
    return uv * scale + (1.0 - scale) * 0.5;
}

fn hdr(c: vec3<f32>, f: Frag, p: vec4<f32>) -> vec3<f32> {
    var out = c;
    if (p.w > 0.5) {
        let bloom = textureSample(t_bloom, s_color, f.uv).rgb;
        out += bloom * p.x;
        if (p.y > 0.0) {
            out += bloom * textureSample(t_dirt, s_color, dirt_uv(f.uv)).rgb * p.y;
        }
    }
    return out;
}

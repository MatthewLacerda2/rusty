// The Hi-Z pyramid (#1963, `render::hiz::pyramid`): the view's depth after the
// phase-one draws, max-reduced level by level for `hiz.wgsl`'s `cs_test`.

// The pyramid's reduction: each texel the farthest depth of the 2x2 below it,
// clamped at the edge, so a level of `ceil(n / 2)` texels covers all `n`.
@group(0) @binding(0) var depth_in: texture_depth_2d;
@group(0) @binding(1) var level_in: texture_2d<f32>;
@group(0) @binding(2) var level_out: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8)
fn cs_reduce_depth(@builtin(global_invocation_id) id: vec3<u32>) {
    let out = textureDimensions(level_out);
    if any(id.xy >= out) {
        return;
    }
    let last = textureDimensions(depth_in) - vec2<u32>(1u);
    var far = 0.0;
    for (var k = 0u; k < 4u; k++) {
        let p = min(id.xy * 2u + vec2<u32>(k & 1u, k >> 1u), last);
        far = max(far, textureLoad(depth_in, p, 0));
    }
    textureStore(level_out, id.xy, vec4<f32>(far, 0.0, 0.0, 0.0));
}

@compute @workgroup_size(8, 8)
fn cs_reduce(@builtin(global_invocation_id) id: vec3<u32>) {
    let out = textureDimensions(level_out);
    if any(id.xy >= out) {
        return;
    }
    let last = textureDimensions(level_in) - vec2<u32>(1u);
    var far = 0.0;
    for (var k = 0u; k < 4u; k++) {
        let p = min(id.xy * 2u + vec2<u32>(k & 1u, k >> 1u), last);
        far = max(far, textureLoad(level_in, p, 0).r);
    }
    textureStore(level_out, id.xy, vec4<f32>(far, 0.0, 0.0, 0.0));
}

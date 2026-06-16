// assets/shaders/decals.wgsl — URP-style box-projector decals (issue #75).
//
// Each decal is an oriented box. The vertex shader draws the box (its BACK faces,
// so a fragment is produced even when the camera is inside the volume). For every
// covered fragment the fragment shader:
//   1. reads the scene depth at this pixel and reconstructs the underlying
//      surface's world position (inverse view-projection),
//   2. transforms that point into the box's local space (unit cube [-0.5,0.5]³),
//   3. rejects it if it falls outside the box (so the decal only marks geometry
//      the projector actually overlaps — it wraps curves, doesn't float),
//   4. projects the decal texture from the box's local XY (the projection runs
//      along local Z), samples it, and alpha-blends the tinted texel over the lit
//      colour. An edge fade on the local Z axis hides the box's hard caps.
//
// This is the forward analogue of a deferred decal: it needs no G-buffer, only the
// depth target the forward pass already wrote.

struct DecalGlobals {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
};

struct DecalUniform {
    model: mat4x4<f32>,
    inv_model: mat4x4<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: DecalGlobals;
@group(1) @binding(0) var<uniform> decal: DecalUniform;
@group(2) @binding(0) var t_depth: texture_depth_2d;
@group(3) @binding(0) var decal_tex: texture_2d<f32>;
@group(3) @binding(1) var decal_sampler: sampler;

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vs_main(@location(0) local_pos: vec3<f32>) -> VsOut {
    let world = decal.model * vec4<f32>(local_pos, 1.0);
    var out: VsOut;
    out.clip_position = globals.view_proj * world;
    return out;
}

// Point-load the scene depth at integer pixel coords (depth targets aren't
// filterable, so we can't textureSample them).
fn load_depth(coord: vec2<i32>) -> f32 {
    return textureLoad(t_depth, coord, 0);
}

// Reconstruct the surface world position from a screen uv [0,1] + a depth in [0,1].
fn world_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let world = globals.inv_view_proj * ndc;
    return world.xyz / world.w;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // This fragment's own screen uv comes from the interpolated clip position.
    let dim = vec2<f32>(textureDimensions(t_depth));
    let screen_ndc = in.clip_position.xy / dim; // [0,1] (clip_position.xy is in px)
    let pixel = vec2<i32>(in.clip_position.xy);

    // 1. Read the scene depth here and reconstruct the surface world position.
    let scene_depth = load_depth(pixel);
    // Skybox / cleared depth (== 1.0) has no surface to mark — discard.
    if (scene_depth >= 1.0) {
        discard;
    }
    let surface = world_from_depth(screen_ndc, scene_depth);

    // 2. Bring the surface point into the box's local space (unit cube).
    let local4 = decal.inv_model * vec4<f32>(surface, 1.0);
    let local = local4.xyz;

    // 3. Reject anything outside the projector box.
    if (any(local < vec3<f32>(-0.5)) || any(local > vec3<f32>(0.5))) {
        discard;
    }

    // 4. Project the texture from local XY (projection axis is local Z). Flip Y so
    //    the stamp is upright; map [-0.5,0.5] -> [0,1].
    let uv = vec2<f32>(local.x + 0.5, 0.5 - local.y);
    var texel = textureSample(decal_tex, decal_sampler, uv);

    // Soft fade toward the box's Z caps so the projector edge isn't a hard line.
    let z_fade = 1.0 - smoothstep(0.35, 0.5, abs(local.z));
    texel = texel * decal.color;
    texel.a = texel.a * z_fade;

    if (texel.a <= 0.001) {
        discard;
    }
    return texel;
}

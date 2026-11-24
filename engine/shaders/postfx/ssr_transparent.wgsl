// SSR's *Mixed* tracing for the translucents: `fs_ray` traces from the nearest `receive_ssr`
// translucent surface (`shader.wgsl`'s `fs_ssr_transparent`, at the rays' size) and shades its
// hit as an opaque pixel's ray is shaded. The forward pass blends it in where its own march
// missed (`forward::transparent::reflect_ssr`).

#import fullscreen
#import common::fog_factor
#import lighting::ssr_surface
#import rt::trace::{trace_closest, hit_found}
#import postfx::ssr_shade::{shade, RAY_OFFSET, REACH_FADE}
#import postfx::ssr_view::{params, view_forward, world_at}

// Octahedral normal, roughness, view depth (0: no surface).
@group(1) @binding(0) var t_surface: texture_2d<f32>;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

// rgb radiance, a its weight: the reach's last fifth fades, as an opaque ray's merge does.
@fragment
fn fs_ray(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let s = textureLoad(t_surface, vec2<i32>(pos.xy), 0);
    if (s.w <= 0.0 || 1.0 - s.z < params.rays.x) {
        return vec4<f32>(0.0);
    }
    let uv = pos.xy / vec2<f32>(textureDimensions(t_surface));
    let world = world_at(uv, s.w, view_forward());
    let eye = params.camera_pos.xyz;
    let n = ssr_surface::oct_decode(s.xy);
    let r = reflect(normalize(world - eye), n);
    // As the forward pass's march leaves it: off the side the ray leaves from.
    let origin = world + faceForward(n, -r, n) * RAY_OFFSET;
    let reach = params.rays.y;
    let hit = trace_closest(origin, r, reach);
    if (!hit_found(hit)) {
        return vec4<f32>(0.0);
    }
    // The scene fog over the line from the eye to the hit, as an opaque pixel's ray takes it.
    let fog = fog_factor(params.fog, origin + r * hit.t, eye);
    let c = mix(shade(hit, origin, r, distance(world, eye)), params.fog.color, fog);
    let w = clamp((reach - hit.t) / (reach * REACH_FADE), 0.0, 1.0);
    // Past f16, `inf`: no reflection, as the march's.
    return select(vec4<f32>(0.0), vec4<f32>(c, w), all(c < vec3<f32>(65000.0)));
}

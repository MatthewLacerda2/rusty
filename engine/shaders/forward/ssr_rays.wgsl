// SSR's *Mixed* for the translucents (`postfx/ssr_transparent.wgsl` traces): `t_ssr_rays`
// layer 0 the nearest one's surface, 1 its ray, 2 the ray its neighbours bound
// (`postfx/ssr_transparent_bound.wgsl`), all at the rays' size.

#define_import_path forward::ssr_rays

#import forward::bindings::{camera, t_opaque, t_ssr_rays}
#import lighting::ssr_surface

// A texel counts on the surface it was traced from only: depth share, normal cosine, as
// `postfx/ssr_rays.wgsl`'s `shared_ray` tests an opaque pixel's.
const SAME_DEPTH: f32 = 0.05;
const SAME_NORMAL: f32 = 0.9;
// A surface texel's roughness when its ray leaves below the surface (`surface_texel`).
const NO_RAY: f32 = -1.0;
const BOUNDED: i32 = 2;

// Depth along the view axis, as the velocity target holds it.
fn view_depth(world: vec3<f32>) -> f32 {
    return dot(camera.clusters.view_z.xyz, world) + camera.clusters.view_z.w;
}

// The *Mixed* ray traced from the surface at `world`, bilinear over its texels: rgb radiance,
// a its share. 0 on the one-layer stand-in, so `RayMarching` reads nothing.
fn traced_ray(world: vec3<f32>, N: vec3<f32>, pixel: vec2<f32>) -> vec4<f32> {
    if (textureNumLayers(t_ssr_rays) < 2u) {
        return vec4<f32>(0.0);
    }
    let rays = vec2<i32>(textureDimensions(t_ssr_rays));
    let at = pixel / vec2<f32>(textureDimensions(t_opaque)) * vec2<f32>(rays) - 0.5;
    let base = vec2<i32>(floor(at));
    let f = at - floor(at);
    let depth = view_depth(world);
    var sum = vec3<f32>(0.0);
    var hits = 0.0;
    var share = 0.0;
    var weight = 0.0;
    for (var i = 0; i < 4; i = i + 1) {
        let o = vec2<i32>(i & 1, i >> 1);
        let q = clamp(base + o, vec2<i32>(0), rays - 1);
        let w = select(1.0 - f.x, f.x, o.x == 1) * select(1.0 - f.y, f.y, o.y == 1);
        let s = textureLoad(t_ssr_rays, q, 0, 0);
        let near_depth = s.w > 0.0 && abs(s.w - depth) <= SAME_DEPTH * depth;
        if (!near_depth || w <= 0.0 || dot(ssr_surface::oct_decode(s.xy), N) < SAME_NORMAL) {
            continue;
        }
        let ray = textureLoad(t_ssr_rays, q, BOUNDED, 0);
        weight += w;
        if (ray.a > 0.0) {
            sum += ray.rgb * w;
            share += ray.a * w;
            hits += w;
        }
    }
    if (hits <= 0.0) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(sum / hits, share / weight);
}

// `shader.wgsl`'s `fs_ssr_transparent`: octahedral normal, roughness, view depth. The fade
// gives it nothing, or a nearer opaque (the copy's alpha, its eye distance) hides it: none.
// Where the shaded normal turns the reflection below the geometric surface `G`, as a rippled
// pool's often does, roughness is `NO_RAY`: the texel stays the surface, its ray traces nothing
// (an opaque pixel's rule, `postfx/ssr_rays.wgsl`'s `fs_ray`), so it never reflects the floor
// under the water.
fn surface_texel(world: vec3<f32>, N: vec3<f32>, G: vec3<f32>, roughness: f32) -> vec4<f32> {
    let fade = ssr_surface::smoothness_fade(roughness, camera.ssr.z, camera.ssr.w);
    let clip = camera.view_proj * vec4<f32>(world, 1.0);
    let uv = vec2<f32>(clip.x / clip.w * 0.5 + 0.5, 0.5 - clip.y / clip.w * 0.5);
    let size = textureDimensions(t_opaque);
    let px = min(vec2<u32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * vec2<f32>(size)), size - 1u);
    let hidden = textureLoad(t_opaque, px, 0).a < distance(camera.camera_pos, world);
    if (fade <= 0.0 || hidden) {
        discard;
    }
    let V = normalize(camera.camera_pos - world);
    let below = dot(reflect(-V, N), faceForward(G, -V, G)) < 0.0;
    return vec4<f32>(ssr_surface::oct_encode(N), select(roughness, NO_RAY, below), view_depth(world));
}

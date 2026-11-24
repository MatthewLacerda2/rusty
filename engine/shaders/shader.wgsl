#import common::{DecalSurface, PI, VertexInput, blend_joints, cluster_index, local_light_radiance}
#import forward::bindings::{bones, camera, cluster_lights, cluster_ranges, decals, draw, instances, lighting, local_lights, material, normal_to_world, objects, s_decal, s_diffuse, s_emissive, s_metallic, s_normal, s_roughness, s_shadow, shadow_tiles, t_ao, t_decal_color, t_decal_data, t_diffuse, t_emissive, t_lightmap_dirs, t_lightmaps, t_metallic, t_normal, t_refl_cube, t_roughness, t_shadow, t_shadow_atlas, t_skybox, tangent_to_world, vertex_to_world, vertex_to_world_prev}
#import lighting::ssr_surface
#import forward::transparent
#import forward::layers
#import forward::ssr_rays
#import forward::mapping
#import forward::{detail, parallax}
#import forward::reflection::{baked_diffuse, environment_brdf, environment_reflection, specular_occlusion}
#import forward::lighting::{ambient_occlusion, sun_shadow_parts, flat_ambient, fold_decals, indirect_bounce, lamp, lightmap_level, probe_irradiance, ssil_realtime, with_ssil_share}
#import forward::shadowmask::{baked_occlusion, directional_shadows, local_shadows}
#import forward::facing::{facing_normal, facing_tangent, seen_from_behind}
#import forward::specular_aa
#import forward::lightmap::lightmap_light
#import forward::motion::{motion_texel}
#import forward::shadow_caster::{casters, shadow_light}

struct VertexOutput {
    @invariant @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) world_tangent: vec4<f32>,
    @location(4) @interpolate(flat) instance: vec3<u32>,
    @location(5) lightmap_uv: vec2<f32>,
};

@vertex
fn vs_main(model: VertexInput, @builtin(instance_index) instance: u32) -> VertexOutput {
    return forward_vertex(model, instance);
}

fn forward_vertex(model: VertexInput, instance: u32) -> VertexOutput {
    var out: VertexOutput;
    let to_world = vertex_to_world(model, instance);
    let world_pos = to_world * vec4<f32>(model.position, 1.0);
    let moved = surface_vertex(world_vertex(model, to_world, world_pos), camera.time);

    out.world_position = moved.position;
    out.world_normal = moved.normal;
    out.tex_coords = model.tex_coords;
    out.world_tangent = vec4<f32>(moved.tangent, model.tangent.w);
    // `w` as transformed and `@invariant`: every pass reproduces the prepass depth bit for bit,
    // which primed passes test `LessEqual`.
    out.clip_position = camera.view_proj * vec4<f32>(moved.position, world_pos.w);
    let drawn = instances[instance];
    out.instance = vec3<u32>(drawn.object, drawn.reflection_probe, drawn.proxy_volume);
    let st = objects[drawn.object].lightmap_st;
    out.lightmap_uv = model.lightmap_uv * st.xy + st.zw;
    return out;
}

struct SurfaceVertex {
    position: vec3<f32>,
    normal: vec3<f32>,
    tangent: vec3<f32>,
};

fn surface_vertex(v: SurfaceVertex, time: f32) -> SurfaceVertex {
    return v;
}

fn world_vertex(model: VertexInput, to_world: mat4x4<f32>, world_pos: vec4<f32>) -> SurfaceVertex {
    let normal = normal_to_world(to_world, model.normal);
    return SurfaceVertex(world_pos.xyz, normal, tangent_to_world(to_world, model.tangent.xyz, normal));
}

fn prepass_clip(in: VertexOutput) {
    let m = material();
    var alpha = m.color_tint.a;
    if (m.use_texture == 1u) {
        let maps = mapping::coords(in.tex_coords, in.world_position, normalize(in.world_normal));
        alpha *= mapping::sample(t_diffuse, s_diffuse, maps).a;
    }
    if (m.use_cutout == 1u && alpha < m.alpha_cutoff) {
        discard;
    }
}

@fragment
fn fs_prepass(in: VertexOutput) {
    prepass_clip(in);
}

// Fills only what a clip reads; the tangent stays zero.
@vertex
fn vs_shadow(model: VertexInput, @builtin(instance_index) instance: u32) -> VertexOutput {
    let caster = casters[instance];
    let joints = model.joint_indices + vec4<u32>(caster.bone_base);
    let skin = blend_joints(
        bones[joints.x], bones[joints.y], bones[joints.z], bones[joints.w],
        model.joint_weights,
    );
    let to_world = caster.model * skin;
    let world = to_world * vec4<f32>(model.position, 1.0);
    let moved = surface_vertex(world_vertex(model, to_world, world), camera.time);
    var out: VertexOutput;
    out.clip_position = shadow_light.light_space * vec4<f32>(moved.position, world.w);
    out.world_position = moved.position;
    out.world_normal = moved.normal;
    out.tex_coords = model.tex_coords;
    out.instance = vec3<u32>(instance, 0u, 0u);
    return out;
}

fn shadow_clip(in: VertexOutput) {
    let caster = casters[in.instance.x];
    // Sampled whatever `use_texture` says: it is per instance, so a branch on it is not uniform.
    let texel = mapping::sample_any(t_diffuse, s_diffuse, caster.terms, in.tex_coords, in.world_position, normalize(in.world_normal)).a;
    if (select(1.0, texel, caster.use_texture == 1u) < caster.alpha_cutoff) {
        discard;
    }
}

@fragment
fn fs_shadow(in: VertexOutput) {
    shadow_clip(in);
}

@fragment
fn fs_gizmo(in: VertexOutput) -> @location(0) vec4<f32> {
    return material().color_tint;
}

@vertex
fn vs_gizmo_vertex(model: VertexInput, @builtin(instance_index) instance: u32) -> VertexOutput {
    var out = forward_vertex(model, instance);
    out.world_tangent = model.tangent;
    return out;
}

@fragment
fn fs_gizmo_vertex(in: VertexOutput) -> @location(0) vec4<f32> {
    return material().color_tint * in.world_tangent;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return with_ssil_share(transparent::finite_hdr(surface_color(in)));
}

@fragment
fn fs_transparent(in: VertexOutput) -> transparent::ReactiveTargets {
    return transparent::reactive_targets(with_ssil_share(transparent::finite_hdr(surface_color(in))));
}

struct VelocityVertex {
    @invariant @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) world_tangent: vec4<f32>,
    @location(4) @interpolate(flat) instance: vec3<u32>,
    @location(5) lightmap_uv: vec2<f32>,
    @location(6) clip_now: vec4<f32>,
    @location(7) clip_prev: vec4<f32>,
};

struct VelocityTargets {
    @location(0) color: vec4<f32>,
    @location(1) velocity: vec4<f32>,
};

@vertex
fn vs_velocity(model: VertexInput, @builtin(instance_index) instance: u32) -> VelocityVertex {
    let v = forward_vertex(model, instance);
    let prev_to_world = vertex_to_world_prev(model, instance);
    let prev_world = prev_to_world * vec4<f32>(model.position, 1.0);
    let prev = surface_vertex(world_vertex(model, prev_to_world, prev_world), camera.stack.y);
    return VelocityVertex(
        v.clip_position, v.world_position, v.world_normal, v.tex_coords,
        v.world_tangent, v.instance, v.lightmap_uv,
        v.clip_position, camera.prev_view_proj * vec4<f32>(prev.position, prev_world.w),
    );
}

@fragment
fn fs_velocity(in: VelocityVertex) -> VelocityTargets {
    return velocity_targets(in);
}

fn velocity_targets(in: VelocityVertex) -> VelocityTargets {
    let v = VertexOutput(
        in.clip_position, in.world_position, in.world_normal, in.tex_coords,
        in.world_tangent, in.instance, in.lightmap_uv,
    );
    return VelocityTargets(with_ssil_share(transparent::finite_hdr(surface_color(v))), velocity_texel(in));
}

fn velocity_texel(in: VelocityVertex) -> vec4<f32> {
    return motion_texel(in.clip_now, in.clip_prev, in.world_position);
}

// HDRP's Transparent Depth Postpass: after the translucents, a surface that asks writes its
// depth and velocity, so depth of field and motion blur see it.
struct DepthPostpassTargets {
    @location(1) velocity: vec4<f32>,
};

@fragment
fn fs_depth_postpass(in: VelocityVertex) -> DepthPostpassTargets {
    return DepthPostpassTargets(velocity_texel(in));
}

var<private> ssr_texels: ssr_surface::SsrTexels;

struct VelocitySsrTargets {
    @location(0) color: vec4<f32>,
    @location(1) velocity: vec4<f32>,
    @location(2) ssr_weight: vec4<f32>,
    @location(3) ssr_applied: vec4<f32>,
};

@fragment
fn fs_velocity_ssr(in: VelocityVertex) -> VelocitySsrTargets {
    let t = velocity_targets(in);
    return VelocitySsrTargets(t.color, t.velocity, ssr_texels.weight, ssr_texels.applied);
}

// *Mixed*'s translucent prepass: the surface a ray leaves from.
@fragment
fn fs_ssr_transparent(in: VertexOutput) -> @location(0) vec4<f32> {
    let v = surface_in(in);
    let b = layers::base(v);
    let l = layers::lit(v, b);
    var N = l.surface.normal;
    if (material().transmission.w > 0.0) {
        N = faceForward(N, -l.V, N);
    }
    return ssr_rays::surface_texel(in.world_position, N, b.geometric, l.surface.roughness);
}

fn surface_in(in: VertexOutput) -> layers::VertexIn {
    return layers::VertexIn(in.world_position, in.world_normal, in.tex_coords, in.world_tangent, in.lightmap_uv);
}

fn surface_color(in: VertexOutput) -> vec4<f32> {
    let m = material();
    let b = layers::base(surface_in(in));
    var base_color = b.base_color;
    let behind = b.behind;
    let geometric = b.geometric;
    let lightmap_lod = b.lightmap_lod;
    let maps = b.maps;
    if (m.is_lit == 0u) {
        return vec4<f32>(transparent::fogged(base_color.rgb, in.world_position), base_color.a);
    }
    let l = layers::lit(surface_in(in), b);
    var specular_kernel = l.specular_kernel;
    let V = l.V;
    let cluster = l.cluster;
    var surface = l.surface;
    var N = surface.normal;
    var albedo = surface.albedo;
    if (m.blend_mode == 3u) {
        albedo = mix(vec3<f32>(1.0), albedo, base_color.a);
    }
    // Preserve specular (URP's `_ALPHAPREMULTIPLY_ON`): only the diffuse takes alpha here, and
    // the blend is premultiplied, so highlights and reflections keep full strength.
    let diffuse_albedo = albedo * select(1.0, base_color.a, m.premultiply_diffuse == 1u);
    let translucent = m.transmission.w > 0.0;
    if (translucent) {
        N = faceForward(N, -V, N);
    }
    let metallic = surface.metallic;
    // Only highlights take the widened roughness; the environment BRDF's weight, opacity and
    // transparent SSR's fade read `surface.roughness`.
    let roughness = specular_aa::widen(surface.roughness, specular_kernel);

    // A dielectric's F0 comes from its ior and `KHR_materials_specular`, computed once on the
    // CPU (`MaterialAsset::dielectric_f0`).
    let dielectric = mapping::dielectric_reflectance(maps);
    let F0 = mix(dielectric.rgb, albedo, metallic);
    // Written as `reflectivity` is below, not `mix`: at the default F90 of 1 it is exactly 1
    // for any `metallic`.
    let F90 = dielectric.w + (1.0 - dielectric.w) * metallic;
    let dielectric_reflectivity = max(dielectric.r, max(dielectric.g, dielectric.b));

    var ambient_irradiance: vec3<f32>;
    var lightmap_specular = vec3<f32>(0.0);
    let lightmap_page = objects[in.instance.x].lightmap_page;
    if (lightmap_page > 0u) {
        let baked = lightmap_light(in.lightmap_uv, lightmap_page - 1u, lightmap_lod, objects[in.instance.x].lightmap_directional == 1u, behind, N, geometric, V, F0, F90, roughness);
        ambient_irradiance = baked.irradiance;
        lightmap_specular = baked.specular;
    } else if (objects[in.instance.x].use_sh == 1u) {
        // `probe_irradiance` returns E; the lightmap and flat ambient already hold E / π.
        ambient_irradiance = probe_irradiance(in.world_position, N, in.instance) / PI;
    } else {
        ambient_irradiance = flat_ambient(N);
    }
    let ao = min(ambient_occlusion(in.clip_position.xy), mapping::occlusion(maps)) * surface.occlusion;
    // The diffuse share is URP's `albedo * oneMinusReflectivity`: what the environment
    // reflection takes, a dielectric's reflectivity, is not diffused too.
    var lighting_color = (ambient_irradiance + indirect_bounce(in.clip_position.xy)) * diffuse_albedo * ((1.0 - dielectric_reflectivity) * (1.0 - metallic)) * ao;
    lighting_color += lightmap_specular * specular_occlusion(saturate(abs(dot(N, V))), ao, roughness);

    let skip_baked = lightmap_page > 0u;

    let baked_shadow = baked_occlusion(in.lightmap_uv, lightmap_page, lightmap_lod, in.instance.x);
    var shadow = vec2<f32>(1.0, 1.0);
    var back_shadow = vec2<f32>(1.0, 1.0);
    if (m.receive_shadows == 1u) {
        shadow = sun_shadow_parts(in.world_position, N);
        if (translucent) {
            back_shadow = sun_shadow_parts(in.world_position, -N);
        }
    }
    for (var i = 0u; i < lighting.num_dir_lights; i = i + 1u) {
        let sun = lighting.dir_lights[i];
        if (skip_baked && sun.baked > 0.5) {
            continue;
        }
        let L = normalize(-sun.direction);
        let radiance = sun.color * sun.intensity;
        let shadowed = directional_shadows(i, shadow, back_shadow, sun.mask_channel, baked_shadow);
        lighting_color += lamp(N, V, L, radiance * shadowed.x, radiance * shadowed.y, F0, F90, metallic, roughness, diffuse_albedo, m.transmission, sun.bounce_baked);
    }

    let range = cluster_ranges[cluster];
    for (var i = 0u; i < range.y; i = i + 1u) {
        let light = local_lights[cluster_lights[range.x + i]];
        if (skip_baked && light.baked > 0.5) {
            continue;
        }
        let radiance = local_light_radiance(light, in.world_position, 0.0);
        if (all(radiance == vec3<f32>(0.0))) {
            continue;
        }
        let lit = local_shadows(light, in.world_position, N, m.receive_shadows == 1u, translucent, baked_shadow);
        let L = normalize(light.position - in.world_position);
        lighting_color += lamp(N, V, L, radiance * lit.x, radiance * lit.y, F0, F90, metallic, roughness, diffuse_albedo, m.transmission, light.bounce_baked);
    }

    let R = reflect(-V, N);
    let env_brdf = environment_brdf(F0, F90, dielectric_reflectivity, metallic, surface.roughness, N, V);
    let reflection_weight = env_brdf * specular_occlusion(saturate(abs(dot(N, V))), ao, roughness);
    let reflection = transparent::reflect_ssr(environment_reflection(in.world_position, R, N, roughness, in.instance.y, baked_diffuse(ambient_irradiance, lightmap_page > 0u || objects[in.instance.x].use_sh == 1u)) * reflection_weight, reflection_weight, in.world_position, N, R, surface.roughness, in.clip_position.xy);
    lighting_color += reflection;
    let surface_fog = transparent::surface_fog(in.world_position);
    ssr_texels = ssr_surface::encode(N, roughness, reflection, reflection_weight, surface_fog);
    if (material().receive_ssr == 0u) {
        ssr_texels = ssr_surface::scale(ssr_texels, vec3<f32>(0.0));
    }

    var emissive = m.emissive.rgb;
    if (m.use_emissive_map == 1u) {
        emissive *= mapping::sample(t_emissive, s_emissive, maps).rgb;
    }
    lighting_color += emissive;
    ssil_realtime += emissive * f32(lightmap_page == 0u && objects[in.instance.x].in_bake == 0u);
    ssil_realtime *= (1.0 - surface_fog);

    return transparent::composite(lighting_color, base_color.a, env_brdf, in.world_position, N, in.clip_position.xy);
}

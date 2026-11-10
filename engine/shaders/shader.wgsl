#import common::{DecalSurface, PI, VertexInput, blend_joints, cluster_index, local_light_radiance}
// Every forward binding but `shadow` (the cascades, read only by `sun_shadow_parts`;
// `fs_main` names its result `shadow`), so a surface variant's blocks can read any,
// with one exception by stage: `instances` is `VERTEX`-only (the fragment stage is at
// its 8 storage buffers), so a fragment block reads its instance through
// `in.instance` and its object record through `objects[in.instance.x]`.
#import forward::bindings::{bones, camera, cluster_lights, cluster_ranges, decals, draw, instances, lighting, local_lights, material, normal_to_world, objects, s_decal, s_diffuse, s_emissive, s_lightmaps, s_metallic, s_normal, s_refl_cube, s_roughness, s_shadow, s_skybox, shadow_tiles, t_ao, t_decal_color, t_decal_data, t_diffuse, t_emissive, t_lightmap_dirs, t_lightmaps, t_metallic, t_normal, t_refl_cube, t_roughness, t_shadow, t_shadow_atlas, t_skybox, tangent_to_world, vertex_to_world, vertex_to_world_prev}
#import lighting::ssr_surface
#import forward::transparent
#import forward::mapping
#import forward::reflection::{baked_diffuse, environment_reflection, specular_occlusion}
#import forward::lighting::{ambient_occlusion, sun_shadow_parts, flat_ambient, fold_decals, indirect_bounce, lamp, lightmap_level, probe_irradiance, ssil_realtime, with_ssil_share}
#import forward::shadowmask::{baked_occlusion, directional_shadows, local_shadows}
#import forward::facing::{facing_normal, facing_tangent, seen_from_behind}
#import forward::specular_aa
#import forward::lightmap::lightmap_light
#import forward::motion::{motion_texel}
#import forward::shadow_caster::{casters, shadow_light}

// The forward pass (and its SSAO prepass and shadow-cascade entry points). Its
// resources are `forward/bindings.wgsl`, its lighting helpers `forward/lighting.wgsl`.

struct VertexOutput {
    // Invariant, so the SSAO prepass, the opaque pass, its motion-vector twin and a
    // surface variant compute bit-identical depth from the same inputs: a primed
    // forward pass tests the prepass's depth `LessEqual` (#1499).
    @invariant @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    // World-space tangent for normal mapping; `w` carries the handedness sign so the
    // fragment shader can reconstruct the bitangent as `w * cross(N, T)`.
    @location(3) world_tangent: vec4<f32>,
    // What the fragment stage reads of its instance (#470, #1706): x its object's
    // slot in `objects` (probe SH, lightmap), y its reflection probes, z its proxy
    // volume. The vertex stage reads `instances`; the fragment stage never does. A
    // shadow caster's x is its index in `casters`.
    @location(4) @interpolate(flat) instance: vec3<u32>,
    // Where this fragment sits in its lightmap page (#438), already scaled/offset.
    @location(5) lightmap_uv: vec2<f32>,
};

@vertex
fn vs_main(model: VertexInput, @builtin(instance_index) instance: u32) -> VertexOutput {
    return forward_vertex(model, instance);
}

// The forward vertex stage, shared by `vs_main` and the motion-vector `vs_velocity`.
fn forward_vertex(model: VertexInput, instance: u32) -> VertexOutput {
    var out: VertexOutput;
    // Skin, then the draw's and the instance's transforms (`vertex_to_world`).
    let to_world = vertex_to_world(model, instance);
    let world_pos = to_world * vec4<f32>(model.position, 1.0);
    // A surface variant's vertex blocks move it here (#1524), at this frame's game time.
    let moved = surface_vertex(world_vertex(model, to_world, world_pos), camera.time);

    out.world_position = moved.position;
    out.world_normal = moved.normal;
    out.tex_coords = model.tex_coords;
    // The tangent's `w` (handedness) passes through untouched.
    out.world_tangent = vec4<f32>(moved.tangent, model.tangent.w);
    // `w` as transformed, so the identity `surface_vertex` leaves the clip position
    // bit-identical to `view_proj * world_pos`.
    out.clip_position = camera.view_proj * vec4<f32>(moved.position, world_pos.w);
    let drawn = instances[instance];
    out.instance = vec3<u32>(drawn.object, drawn.reflection_probe, drawn.proxy_volume);
    let st = objects[drawn.object].lightmap_st;
    out.lightmap_uv = model.lightmap_uv * st.xy + st.zw;
    return out;
}

// A vertex as a surface variant's vertex blocks see it (#1524): its world position,
// normal and tangent. `surface_vertex` is the identity here; a bake whose recipe holds
// a vertex-stage block (`vertex_waves`) replaces its body with the blocks' chain
// (`shadergen::assemble`). Every vertex stage that draws a mesh calls it: the forward
// one (so the SSAO prepass too), the motion-vector one again at last frame's game time,
// and the shadow casters', so depth, shadows and motion follow the moved surface.
struct SurfaceVertex {
    position: vec3<f32>,
    normal: vec3<f32>,
    tangent: vec3<f32>,
};

fn surface_vertex(v: SurfaceVertex, time: f32) -> SurfaceVertex {
    return v;
}

// `model`'s vertex through `to_world` (`world_pos` already is its position): the
// normal by the inverse-transpose, the tangent along the surface (#1596).
fn world_vertex(model: VertexInput, to_world: mat4x4<f32>, world_pos: vec4<f32>) -> SurfaceVertex {
    let normal = normal_to_world(to_world, model.normal);
    return SurfaceVertex(world_pos.xyz, normal, tangent_to_world(to_world, model.tangent.xyz, normal));
}

// The SSAO depth prepass (#436): depth only, so it writes no colour. Cutout texels
// are clipped as `fs_main` clips them, so they cast no occlusion. Unlit draws (gizmos,
// a light's own mesh) never reach it: `BatchKey::in_prepass` leaves them out on the
// CPU, the one rule depth priming reads too (#1499). A surface variant whose blocks
// cut fragments or move the UVs adds `fs_prepass_cut`, which opens like
// `surface_color`, runs its cuts and then this (#648).
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

// ---- The shadow pass (#435, #648) ----
// The cascades' depth is drawn from this module too, so a surface variant carries
// its own shadow entry points. Its resources sit at bindings the forward pass leaves
// free — group 0 binding 6, group 1 binding 3 — and share `bones`, `draw` and
// `materials` (group 1 bindings 1, 0, 4; #1616: `material()` reads the clip's record). Group 2 is the material's, as
// in the forward pass: only the clipping pipelines bind it.

// The cascade's matrix and the casters: `forward/shadow_caster.wgsl`.

// A caster's vertex in the cascade's light space. Only what a clip reads is filled:
// the UVs, world position and normal (a triplanar clip weighs by it); the tangent
// stays zero.
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
    // The vertex blocks move the caster as `vs_main` moves the surface (#1524).
    let moved = surface_vertex(world_vertex(model, to_world, world), camera.time);
    var out: VertexOutput;
    out.clip_position = shadow_light.light_space * vec4<f32>(moved.position, world.w);
    out.world_position = moved.position;
    out.world_normal = moved.normal;
    out.tex_coords = model.tex_coords;
    out.instance = vec3<u32>(instance, 0u, 0u);
    return out;
}

// A cutout caster's shadow is clipped as its surface is (#648), at the texels its
// material's Tiling/Offset and world mapping read (#1594). The albedo is sampled
// unconditionally: `use_texture` and the mapping are per instance, so a branch on
// either is not uniform (`mapping::sample_any`).
fn shadow_clip(in: VertexOutput) {
    let caster = casters[in.instance.x];
    let texel = mapping::sample_any(t_diffuse, s_diffuse, caster.terms, in.tex_coords, in.world_position, normalize(in.world_normal)).a;
    if (select(1.0, texel, caster.use_texture == 1u) < caster.alpha_cutoff) {
        discard;
    }
}

@fragment
fn fs_shadow(in: VertexOutput) {
    shadow_clip(in);
}

// The editor's line gizmos (grid, collider boxes, axis arrows, probes): drawn on the
// finished image after post-FX (#1224), so the authored colour as is, unlit and
// unfogged, as Unity draws its gizmos. Fog is a scene effect; a gizmo is not scene.
@fragment
fn fs_gizmo(in: VertexOutput) -> @location(0) vec4<f32> {
    return material().color_tint;
}

// The collider and debug-line gizmos (#1882): `fs_gizmo`'s lines, each vertex its own
// colour, carried in the vertex's `tangent` (lines have no tangent basis) and passed
// through untransformed, so one draw holds lines of every colour.
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

// SSIL's share (#1750): `forward::lighting::with_ssil_share`.

// ---- TAA's reactive mask (#1845) ----
// A view that runs TAA draws its translucents through this instead of `fs_main`:
// the same colour at location 0, and at location 1 (the `R16Float` reactive
// target, blended by max) how much of the pixel this surface laid down this frame
// (`forward::transparent::reactive_targets`).
@fragment
fn fs_transparent(in: VertexOutput) -> transparent::ReactiveTargets {
    return transparent::reactive_targets(with_ssil_share(transparent::finite_hdr(surface_color(in))));
}

// ---- Motion vectors (#1435) ----
// A view that runs motion blur draws its opaque solids through these instead: the
// same surface colour at location 0, and at location 1 (the `Rgba16Float` velocity
// target) how far the fragment moved on screen since last frame, in NDC, through its
// own last-frame pose and this camera's last-frame view-projection, then its depth
// along this camera's view axis and which camera of the stack drew it (#1436: motion blur's
// depth order, kept per camera). Every field of
// `VertexOutput`, then the two clip positions the velocity is the difference of.
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
    // Last frame's vertex blocks ran at last frame's game time (`stack.y`, #1524), so
    // a surface that only moves in its vertex stage still writes its motion.
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

// What `fs_velocity` writes; `fs_velocity_ssr` writes it too.
fn velocity_targets(in: VelocityVertex) -> VelocityTargets {
    let v = VertexOutput(
        in.clip_position, in.world_position, in.world_normal, in.tex_coords,
        in.world_tangent, in.instance, in.lightmap_uv,
    );
    return VelocityTargets(with_ssil_share(transparent::finite_hdr(surface_color(v))), velocity_texel(in));
}

// The velocity target's texel for `in` (`forward::motion::motion_texel`).
fn velocity_texel(in: VelocityVertex) -> vec4<f32> {
    return motion_texel(in.clip_now, in.clip_prev, in.world_position);
}

// ---- The transparent depth postpass (#1523) ----
// HDRP's *Transparent Depth Postpass*: after the translucents drew, a surface that
// asks for it writes its depth, and in a view with a velocity target its texel
// there (location 1; the pipeline has no colour target at 0), so depth of field and
// motion blur see the surface, not what is behind it. A view without one draws it
// through `fs_prepass`.
struct DepthPostpassTargets {
    @location(1) velocity: vec4<f32>,
};

@fragment
fn fs_depth_postpass(in: VelocityVertex) -> DepthPostpassTargets {
    return DepthPostpassTargets(velocity_texel(in));
}

// ---- The SSR surface targets (#1526) ----
// A view whose camera runs screen-space reflections draws its opaque solids through
// `fs_velocity_ssr`: `fs_velocity`'s two targets, and at locations 2 and 3 the
// surface SSR reads (`lighting/ssr_surface.wgsl`): the shading normal, the weight a
// hit gets and the environment reflection step 5 applied, which a hit replaces.
// `surface_color` records them in `ssr_texels` at step 5; an unlit surface returns
// first and leaves them zero, so it gets no SSR.
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

// The lit surface colour `fs_main` writes: an authored surface variant splices its
// blocks in here (`shadergen::assemble`), so every colour entry point carries them
// (`fs_main`, `fs_velocity`, `fs_velocity_ssr`, `fs_transparent`).
fn surface_color(in: VertexOutput) -> vec4<f32> {
    let m = material();
    // The geometric normal, turned toward the viewer on a face seen from behind when
    // the material draws back faces (#1615, `seen_from_behind`). First, before any branch.
    let behind = seen_from_behind(in.world_normal, in.world_position, m.two_sided == 1u);
    let geometric = facing_normal(in.world_normal, behind);
    // The lightmap's mip level (#1694), from derivatives: before any branch too.
    let lightmap_lod = lightmap_level(in.lightmap_uv);
    // What this pixel's spread of normals adds to the GGX lobe (#1702), before any
    // branch too; `roughness` below is widened by it. Taken before the two-sided flip:
    // a flip inside a pixel quad (a pane seen edge-on) is not a curved surface.
    var specular_kernel = specular_aa::kernel(normalize(in.world_normal));
    // Where every map below is read (#1573): UV0 tiled, or projected from world space.
    let maps = mapping::coords(in.tex_coords, in.world_position, geometric);
    var base_color = m.color_tint;
    if (m.use_texture == 1u) {
        base_color *= mapping::sample(t_diffuse, s_diffuse, maps);
    }

    // Cutout alpha-test (#242): for a Cutout material, drop sub-threshold fragments
    // before any shading so the surface reads as hard-edged (foliage, grates). Opaque
    // and Transparent materials leave `use_cutout == 0` and skip this. Done before the
    // unlit early-out so an unlit cutout (a flat alpha-tested billboard) also clips.
    if (m.use_cutout == 1u && base_color.a < m.alpha_cutoff) {
        discard;
    }

    // Unlit rendering (e.g. path highlights, a light's own mesh). Still fogged: an
    // unlit sign down a smoky corridor fades like the wall it hangs on (#437).
    if (m.is_lit == 0u) {
        return vec4<f32>(transparent::fogged(base_color.rgb, in.world_position), base_color.a);
    }

    // Geometric normal, optionally perturbed by a tangent-space normal map (#207; z
    // rebuilt by `maps::unpack_normal`), rotated into world space by the mesh's tangent
    // frame or, under a world mapping, reoriented per projection (`mapping::sample_normal`).
    // Its spread inside the texel read widens the lobe too (#1702, Toksvig), at the
    // level its footprint covers: derivatives, but the branch is on the draw's record,
    // so uniform, and a material without a normal map pays nothing for them.
    var N = geometric;
    if (m.use_normal_map != 0u) {
        let read = mapping::sample_normal(t_normal, s_normal, maps, N, facing_tangent(in.world_tangent, behind));
        N = read.normal;
        let level = mapping::normal_level(t_normal, maps);
        specular_kernel += specular_aa::map_kernel(m.use_normal_map, m.normal_variance, read.length, level);
    }
    let V = normalize(camera.camera_pos - in.world_position);

    // glTF metallic-roughness packs metallic in BLUE, roughness in GREEN. Each scalar
    // multiplies its map channel when the flag is set, else acts alone (#202, #1574).
    let surface_metallic = clamp(m.metallic * mapping::mask(m.use_metallic_map, t_metallic, s_metallic, maps, 2u), 0.0, 1.0);
    let surface_roughness = clamp(m.roughness * mapping::mask(m.use_roughness_map, t_roughness, s_roughness, maps, 1u), 0.04, 1.0);

    // 0. Surface decals (#638): this cluster's decals change the material inputs
    // before any light is summed, so a decal is lit, shadowed and occluded like the
    // surface it lands on. The angle fade reads the geometric normal.
    let cluster = cluster_index(camera.clusters, camera.view_proj, in.world_position);
    var surface = DecalSurface(base_color.rgb, N, surface_metallic, surface_roughness, 1.0);
    if (m.receive_decals == 1u) {
        surface = fold_decals(surface, cluster, in.world_position, geometric);
    }
    N = surface.normal;
    // A Multiply surface tints what is behind it toward white by its alpha (#1523,
    // URP's `ApplyAlphaModulate` on the albedo).
    var albedo = surface.albedo;
    if (m.blend_mode == 3u) {
        albedo = mix(vec3<f32>(1.0), albedo, base_color.a);
    }
    // Preserve specular (#1523, URP's `_ALPHAPREMULTIPLY_ON`): only the diffuse is
    // multiplied by alpha here, and the pipeline blends premultiplied, so reflections
    // and highlights are not faded with it. `albedo` still sets a metal's F0.
    let diffuse_albedo = albedo * select(1.0, base_color.a, m.premultiply_diffuse == 1u);
    // Thin translucency (#1523) lights the side facing away from a light, so the
    // normal faces the viewer, as HDRP flips a double-sided translucent's normal.
    let translucent = m.transmission.w > 0.0;
    if (translucent) {
        N = faceForward(N, -V, N);
    }
    let metallic = surface.metallic;
    // Specular AA (#1702): every GGX read below (the lights, the lightmap's lobe, the
    // environment reflection's mip and its specular occlusion, SSR's record) takes the
    // widened roughness, so a highlight and its reflection agree. The environment
    // BRDF's weight, and so a transparent surface's opacity, and transparent SSR's
    // smoothness fade read the authored `surface.roughness` (step 5): the widening
    // spreads a highlight and blurs a reflection, never takes a far glossy surface's
    // grazing reflection, its opacity or its SSR away.
    let roughness = specular_aa::widen(surface.roughness, specular_kernel);

    let F0 = mix(vec3<f32>(0.04), albedo, metallic);

    // 1. Ambient lighting term. A lightmapped static mesh reads its lightmap (#438).
    // Non-static objects that a light probe covers
    // (`use_sh == 1`, #240) reconstruct DIRECTIONAL irradiance from their interpolated
    // SH probe; everything else falls back to the flat Hemispherical Sky-Ground
    // gradient. All three are E / pi, so they feed the same albedo * 0.96 * (1 - metallic)
    // Lambert response (the 4% is the dielectric's reflection, step 5).
    var ambient_irradiance: vec3<f32>;
    // A directional lightmap's one specular lobe: the `Baked` lamps' highlight (#1695).
    var lightmap_specular = vec3<f32>(0.0);
    let lightmap_page = objects[in.instance.x].lightmap_page;
    if (lightmap_page > 0u) {
        // Baked lightmap (#438, `forward::lightmap`), at the level taken above.
        let baked = lightmap_light(in.lightmap_uv, lightmap_page - 1u, lightmap_lod, objects[in.instance.x].lightmap_directional == 1u, behind, N, geometric, V, F0, roughness);
        ambient_irradiance = baked.irradiance;
        lightmap_specular = baked.specular;
    } else if (objects[in.instance.x].use_sh == 1u) {
        // `probe_irradiance` returns irradiance E (its one SH sample, or its Light Probe
        // Proxy Volume's grid inside one, #1672); a Lambert surface reflects albedo / pi
        // * E, the same response `calculate_pbr` gives a direct light and the E / pi the
        // lightmap and flat ambient already hold (#807).
        ambient_irradiance = probe_irradiance(in.world_position, N, in.instance) / PI;
    } else {
        ambient_irradiance = flat_ambient(N);
    }
    // SSAO (#436) times the decals' occlusion darkens only the indirect light: ambient
    // here, and the env reflection below through its specular occlusion (#1697).
    let ao = ambient_occlusion(in.clip_position.xy) * surface.occlusion;
    // The diffuse share is URP's `albedo * oneMinusReflectivity`: what the
    // environment reflection (step 5) takes, a dielectric's 4%, is not diffused too.
    // SSIL's bounce (#1750) is indirect diffuse too: the same response, the same AO.
    var lighting_color = (ambient_irradiance + indirect_bounce(in.clip_position.xy)) * diffuse_albedo * (0.96 * (1.0 - metallic)) * ao;
    // The lobe is `Baked` lamps' light, so it stays out of `ssil_realtime`, as the
    // lightmap's own light does.
    lighting_color += lightmap_specular;

    // A lightmapped surface skips `Baked` lights below: their direct light is already
    // in its lightmap (#438). Only a probe/reflection bake capture uploads them at all
    // (#809); a frame draws `Mixed` and `Realtime` lights only (a `Mixed` light a
    // `Shadowmask` bake baked as `Baked` is `Baked` here, #1695).
    let skip_baked = lightmap_page > 0u;

    // 2. Directional lights (#434). Slot 0 is the sun, the one the cascades shadow.
    // A surface that receives no shadows (#1523, URP's *Receive Shadows*) skips the
    // cascades and the atlas.
    // Light through a translucent surface reaches its back face, so its shadow is
    // looked up off that face (`-N`), where the surface casts none on itself.
    // Each shadow is its realtime part and its fade (`sun_shadow_parts`); a
    // `Mixed` light under a shadowmask meets its baked shadow too (#1695,
    // `forward::shadowmask`), which a surface that receives no realtime shadow
    // still gets, as it gets a `Baked` light's from its lightmap.
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
        lighting_color += lamp(N, V, L, radiance * shadowed.x, radiance * shadowed.y, F0, metallic, roughness, diffuse_albedo, m.transmission, sun.bounce_baked);
    }

    // 3. Point and spot lights, only those binned into this fragment's cluster (#434),
    // each shadowed through the atlas when it casts (#468).
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
        // Its shadow (#468, faded by distance #1712), off the back face too for a
        // translucent surface.
        let lit = local_shadows(light, in.world_position, N, m.receive_shadows == 1u, translucent, baked_shadow);
        let L = normalize(light.position - in.world_position);
        lighting_color += lamp(N, V, L, radiance * lit.x, radiance * lit.y, F0, metallic, roughness, diffuse_albedo, m.transmission, light.bounce_baked);
    }

    // 5. Environment reflections (#244), always on (#718) but in a light-probe capture
    // (#1668, `environment_reflection`): like Unity's Lighting ->
    // Environment Reflections, the sky / reflection-probe term applies whether or not
    // the camera runs SSR. Where SSR runs and hits, it replaces exactly this term
    // (postfx), so the record below stores what was added and under what weight.
    // A metal has no diffuse term, so without this it renders black. See
    // `environment_reflection`.
    // Its weight is Unity URP's analytic environment BRDF (#1543; `BRDF.hlsl`
    // `EnvironmentBRDFSpecular`, `GlobalIllumination.hlsl`): roughness only picks the
    // prefiltered mip, and URP's `surfaceReduction`, `1 / (alpha^2 + 1)` with
    // `alpha = roughness^2` (so perceptual roughness to the 4th), is the energy a rough
    // lobe loses, so a rough metal reads blurry, not black. `roughness` is perceptual,
    // URP's `perceptualRoughness`; a dielectric reflects F0 = 0.04 head-on. The angle is
    // `|N.V|`: a normal map or a vertex normal can still lean away from the camera on a
    // face turned toward it, which would otherwise take the grazing Fresnel.
    // The AO weighs it as specular occlusion (#1697, Frostbite's, as HDRP has it), and
    // each probe is normalised by this pixel's baked diffuse (`baked_diffuse`).
    // The weight reads the authored roughness, not the widened one: a far floor's
    // spread of normals (up to 0.08 of α², `specular_aa`) would take `grazing` from
    // 0.96 to 0.51 and a pane's opacity with it (#1702's review). URP has no
    // specular AA; HDRP filters its smoothness through a preintegrated FGD rusty
    // has not got.
    let R = reflect(-V, N);
    let grazing = saturate(1.0 - surface.roughness + (0.04 + 0.96 * metallic)); // URP's reflectivity
    let alpha = surface.roughness * surface.roughness;
    let fresnel = pow(1.0 - saturate(abs(dot(N, V))), 4.0);
    let env_brdf = mix(F0, vec3<f32>(grazing), fresnel) / (alpha * alpha + 1.0);
    let reflection_weight = env_brdf * specular_occlusion(saturate(abs(dot(N, V))), ao, roughness);
    // A transparent surface with `receive_ssr` traces SSR over it here (#1570), faded
    // by its authored smoothness.
    let reflection = transparent::reflect_ssr(environment_reflection(in.world_position, R, N, roughness, in.instance.y, baked_diffuse(ambient_irradiance, lightmap_page > 0u || objects[in.instance.x].use_sh == 1u)) * reflection_weight, reflection_weight, in.world_position, N, R, surface.roughness);
    lighting_color += reflection;
    // One fog value for the SSR record and SSIL's realtime light (step 6): the
    // distance fog's and the volumetric fog's together (#1703).
    let surface_fog = transparent::surface_fog(in.world_position);
    ssr_texels = ssr_surface::encode(N, roughness, reflection, reflection_weight, surface_fog);
    // An opaque material with `receive_ssr` off keeps its probe (#2004): no weight.
    if (material().receive_ssr == 0u) {
        ssr_texels = ssr_surface::scale(ssr_texels, vec3<f32>(0.0));
    }

    // 6. Emissive (#222 factor, #207 map): self-illumination added on top of the lit
    // colour, independent of any light. The factor is modulated by the emissive map's
    // rgb when one is bound (`factor * map.rgb`, the glTF convention). The HDR target +
    // bloom bright-pass turn values >1.0 into a glow with no post-FX work here.
    var emissive = m.emissive.rgb;
    if (m.use_emissive_map == 1u) {
        emissive *= mapping::sample(t_emissive, s_emissive, maps).rgb;
    }
    lighting_color += emissive;
    // A bake holds the emission of every static mesh it took in, lightmap UVs or
    // not (`in_bake`); the fog (step 7) dims the rest.
    ssil_realtime += emissive * f32(lightmap_page == 0u && objects[in.instance.x].in_bake == 0u);
    ssil_realtime *= (1.0 - surface_fog);

    // 7. Scene fog (#437), last, in linear HDR before post-FX, and how a transparent
    // surface composites (#1523). Authored surface blocks fold into `lighting_color`
    // here, so they are fogged too.
    return transparent::composite(lighting_color, base_color.a, env_brdf, in.world_position, N, in.clip_position.xy);
}

#define_import_path forward::layers

#import common::{DecalSurface, cluster_index}
#import forward::bindings::{camera, material, s_diffuse, s_metallic, s_normal, s_roughness, t_diffuse, t_metallic, t_normal, t_roughness}
#import forward::mapping
#import forward::{detail, parallax}
#import forward::lighting::{fold_decals, lightmap_level}
#import forward::facing::{facing_normal, facing_tangent, seen_from_behind}
#import forward::specular_aa

struct VertexIn {
    world_position: vec3<f32>,
    world_normal: vec3<f32>,
    tex_coords: vec2<f32>,
    world_tangent: vec4<f32>,
    lightmap_uv: vec2<f32>,
};

// What `shader.wgsl` reads before the lit/unlit split, and a lit surface's maps and decals.
struct SurfaceBase {
    base_color: vec4<f32>,
    behind: bool,
    geometric: vec3<f32>,
    lightmap_lod: f32,
    specular_kernel: f32,
    maps: mapping::MapCoords,
};

fn base(in: VertexIn) -> SurfaceBase {
    let m = material();
    let behind = seen_from_behind(in.world_normal, in.world_position, m.two_sided == 1u);
    let geometric = facing_normal(in.world_normal, behind);
    let lightmap_lod = lightmap_level(in.lightmap_uv);
    // The unflipped normal: a two-sided flip inside a pixel quad is not curvature.
    let specular_kernel = specular_aa::kernel(normalize(in.world_normal));
    let maps = parallax::displace(mapping::coords(in.tex_coords, in.world_position, geometric), in.world_position, geometric);
    var base_color = m.color_tint;
    if (m.use_texture == 1u) {
        base_color *= mapping::sample(t_diffuse, s_diffuse, maps);
    }

    // Before the unlit early-out, so an unlit cutout billboard clips too.
    if (m.use_cutout == 1u && base_color.a < m.alpha_cutoff) {
        discard;
    }
    return SurfaceBase(base_color, behind, geometric, lightmap_lod, specular_kernel, maps);
}

struct SurfaceLayers {
    specular_kernel: f32,
    V: vec3<f32>,
    cluster: u32,
    surface: DecalSurface,
};

fn lit(in: VertexIn, b: SurfaceBase) -> SurfaceLayers {
    let m = material();
    let maps = b.maps;
    var specular_kernel = b.specular_kernel;
    var N = b.geometric;
    if (m.use_normal_map != 0u) {
        let read = mapping::sample_normal(t_normal, s_normal, maps, N, facing_tangent(in.world_tangent, b.behind));
        N = read.normal;
        let level = mapping::normal_level(t_normal, maps);
        specular_kernel += specular_aa::map_kernel(m.use_normal_map, m.normal_variance, read.length, level);
    }
    let V = normalize(camera.camera_pos - in.world_position);

    let surface_metallic = clamp(m.metallic * mapping::mask(m.use_metallic_map, t_metallic, s_metallic, maps, 2u), 0.0, 1.0);
    let main_roughness = clamp(m.roughness * mapping::mask(m.use_roughness_map, t_roughness, s_roughness, maps, 1u), 0.04, 1.0);
    let detailed = detail::apply(detail::coords(maps, in.world_position, b.geometric), detail::mask(maps), b.base_color.rgb, main_roughness, N, facing_tangent(in.world_tangent, b.behind));

    let cluster = cluster_index(camera.clusters, camera.view_proj, in.world_position);
    var surface = DecalSurface(detailed.albedo, detailed.normal, surface_metallic, detailed.roughness, 1.0);
    if (m.receive_decals == 1u) {
        surface = fold_decals(surface, cluster, in.world_position, b.geometric);
    }
    return SurfaceLayers(specular_kernel, V, cluster, surface);
}

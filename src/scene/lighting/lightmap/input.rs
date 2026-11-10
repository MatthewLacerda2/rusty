//! What the lightmap bake sees (#438): the static scene flattened to world-space
//! triangles, the lights that take part in the bake (`lights`), and the sky. Gathered once from a
//! [`Scene`]; the tracer never touches the ECS, so the bake is a pure function of this.

use std::cell::RefCell;

use glam::{Vec2, Vec3};

pub use super::lights::{BakeLight, LightShape};
use crate::asset::MapRef;
use crate::components::mesh::{clip_runs, material_runs};
use crate::components::{MaterialAsset, MeshComponent, ShadowCasting};
use crate::core::collections::Map;
use crate::scene::Scene;

/// One static mesh in world space. Its occluding runs block and bounce light; the
/// ones with a usable lightmap UV also receive a lightmap, over every drawn run.
/// `cast_shadows` narrows that, as Unity bakes it (#1571): an `Off` mesh is still
/// seen by bounce rays (it bounces and emits) but blocks no light; a `ShadowsOnly`
/// one blocks light but is seen by nothing and gets no lightmap. `scale_in_lightmap`
/// scales its texel density (#1704); at 0 it gets no lightmap either, but still
/// blocks and bounces light.
#[derive(Clone, Debug)]
pub struct BakeMesh {
    /// The entity the lightmap belongs to.
    pub entity: u32,
    pub positions: Vec<Vec3>,
    /// Unit world-space vertex normals.
    pub normals: Vec<Vec3>,
    pub lightmap_uvs: Vec<Vec2>,
    /// The drawn runs' triangles: a run with no material slot is left out, as the
    /// renderer leaves it undrawn (#1290).
    pub indices: Vec<u32>,
    /// How `indices` splits into material runs, in order (#1290).
    pub runs: Vec<BakeRun>,
    /// The mesh's *Cast Shadows* mode (#1571).
    pub cast_shadows: ShadowCasting,
    /// The mesh's *Scale In Lightmap* (#1704): a multiplier on the bake's texel
    /// density, 1 by default.
    pub scale_in_lightmap: f32,
}

impl Default for BakeMesh {
    fn default() -> Self {
        Self {
            entity: 0,
            positions: Vec::new(),
            normals: Vec::new(),
            lightmap_uvs: Vec::new(),
            indices: Vec::new(),
            runs: Vec::new(),
            cast_shadows: ShadowCasting::default(),
            scale_in_lightmap: 1.0,
        }
    }
}

/// One material run of a [`BakeMesh`]: the indices up to `end` (from the previous
/// run's `end`) and the surface its own material gives them.
#[derive(Clone, Copy, Debug)]
pub struct BakeRun {
    pub end: u32,
    /// Diffuse reflectance: the material colour times its albedo map's average.
    pub albedo: Vec3,
    /// Emitted radiance, as the forward shader adds it (a Transparent material's
    /// scaled by its alpha, as it blends).
    pub emissive: Vec3,
    /// Whether the run blocks and bounces light (it is in the bake's BVH), as far
    /// as the mesh's `cast_shadows` lets it. A Transparent run does neither, but still receives its share of the entity's
    /// lightmap, which the renderer samples for it too.
    pub occludes: bool,
}

impl BakeMesh {
    /// Whether the bake gives this mesh a lightmap: a camera sees it (not
    /// `ShadowsOnly`, #1571) and its Scale In Lightmap is above 0 (#1704). One that
    /// doesn't still takes part in the bake as its `cast_shadows` says, and reads
    /// light probes when drawn.
    pub fn receives_lightmap(&self) -> bool {
        self.cast_shadows.is_visible() && self.scale_in_lightmap > 0.0
    }

    /// The texels per world unit a lightmap `edge` texels a side gives this mesh:
    /// beside the density it asked for, what a bake report shows (#1980). `None`
    /// when its lightmap UVs enclose no area.
    pub fn texel_density(&self, edge: u32) -> Option<f32> {
        super::raster::world_per_uv(&self.surface()).map(|w| edge as f32 / w)
    }

    /// The run index `index` (into `indices`) belongs to; the last run past the end.
    pub fn run_at(&self, index: u32) -> &BakeRun {
        let k = self.runs.partition_point(|r| r.end <= index);
        &self.runs[k.min(self.runs.len() - 1)]
    }
}

/// Everything the bake reads.
#[derive(Clone, Debug, Default)]
pub struct BakeScene {
    pub meshes: Vec<BakeMesh>,
    pub lights: Vec<BakeLight>,
    /// The drawn ambient (`Scene::drawn_ambient`, #1647): a ray escaping the scene
    /// sees its `incoming` radiance, so an open texel bakes to the `at` the forward
    /// shader shades an unbaked surface with.
    pub sky: crate::scene::AmbientRadiance,
    /// The scene's diffuse model (`LightingSettings::diffuse_model`, #1698): how a
    /// baked lamp's direct light falls off across a texel or a bounce surface, and how
    /// it lands in a probe, as the forward shader shades a live one.
    pub diffuse: crate::scene::lighting::DiffuseModel,
    /// The shadowmask channels of a `Shadowmask` bake (#1695; empty otherwise),
    /// which `lights` carry and the bake records in `LightmapSet::channels`.
    pub shadowmask: super::lights::MaskChannels,
}

impl BakeScene {
    /// Flatten `scene`: the drawn runs of every active static mesh that is not a light
    /// gizmo, each with its own material (#1290) and the mesh's *Cast Shadows*
    /// (#1571), the active `Mixed` / `Baked` lights,
    /// the ambient sky and the diffuse model. `map_average` turns a texture path into its average linear
    /// colour (`None` when it cannot be read); it is asked once per path.
    pub fn gather(scene: &Scene, map_average: &dyn Fn(&MapRef) -> Option<Vec3>) -> Self {
        let map_average = once_per_path(map_average);
        let mut out = BakeScene {
            sky: scene.drawn_ambient().radiance(),
            diffuse: scene.lighting_settings.diffuse_model,
            ..Default::default()
        };
        for id in scene.world.ids_with_mesh() {
            if !Self::takes_mesh(scene, id) {
                continue;
            }
            let mesh = scene.world.mesh(id).expect("id came from ids_with_mesh");
            let (indices, runs) = drawn_runs(scene, id, &mesh, &map_average);
            if indices.is_empty() {
                continue;
            }
            let matrix = scene.world.local_to_world(id);
            let normal_matrix = matrix.inverse().transpose();
            out.meshes.push(BakeMesh {
                entity: id,
                positions: mesh
                    .vertices
                    .iter()
                    .map(|v| matrix.transform_point3(Vec3::from(v.position)))
                    .collect(),
                normals: mesh
                    .vertices
                    .iter()
                    .map(|v| {
                        let n = normal_matrix.transform_vector3(Vec3::from(v.normal));
                        n.try_normalize().unwrap_or(Vec3::Y)
                    })
                    .collect(),
                lightmap_uvs: mesh.vertices.iter().map(|v| v.lightmap_uv.into()).collect(),
                indices,
                runs,
                cast_shadows: mesh.cast_shadows,
                scale_in_lightmap: mesh.scale_in_lightmap,
            });
        }
        out.gather_lights(scene);
        out
    }
}

impl BakeScene {
    /// Whether [`Self::gather`] takes mesh `id` in: active, static, unskinned and no
    /// light gizmo. Such a mesh occludes, bounces and emits in the bake whether or
    /// not it receives a lightmap, so the renderer's SSIL gathers none of its
    /// emission once the scene holds a bake (#1750, `ObjectData::in_bake`).
    pub fn takes_mesh(scene: &Scene, id: u32) -> bool {
        scene.world.is_active_in_hierarchy(id)
            && scene.world.is_static(id)
            && !scene.world.has_light(id)
            && scene.world.mesh(id).is_some_and(|m| !m.is_skinned())
    }
}

/// `map_average`, asked at most once per path: a map many slots or entities share is
/// decoded once per gather.
fn once_per_path(
    map_average: &dyn Fn(&MapRef) -> Option<Vec3>,
) -> impl Fn(&MapRef) -> Option<Vec3> + '_ {
    let cache = RefCell::new(Map::default());
    move |map: &MapRef| {
        let cached = cache.borrow().get(map).copied();
        cached.unwrap_or_else(|| {
            *cache
                .borrow_mut()
                .entry(map.clone())
                .or_insert_with(|| map_average(map))
        })
    }
}

/// Entity `id`'s drawn index runs, each with its own material's surface, paired
/// with its slots the way the renderer draws them ([`material_runs`]): a run with no
/// slot is left out. A slot past the last run layers over that run as the renderer
/// draws it, judged by its own material: it adds its emission, and an opaque one
/// over a run that let light through makes the run occlude with its albedo.
fn drawn_runs(
    scene: &Scene,
    id: u32,
    mesh: &MeshComponent,
    map_average: &dyn Fn(&MapRef) -> Option<Vec3>,
) -> (Vec<u32>, Vec<BakeRun>) {
    let ranges = clip_runs(mesh.index_ranges(), mesh.indices.len() as u32);
    let slots = scene.world.material(id).map(|m| m.materials.len());
    let (mut indices, mut runs) = (Vec::new(), Vec::<BakeRun>::new());
    let mut previous = None;
    for (slot, range) in material_runs(ranges, slots) {
        let material = slots.and_then(|_| scene.material_entry_at(id, slot).map(|(_, m)| m));
        let run = surface(material, map_average);
        if previous.as_ref() == Some(&range) {
            let last = runs
                .last_mut()
                .expect("a repeated run follows its first draw");
            last.emissive += run.emissive;
            if run.occludes && !last.occludes {
                (last.occludes, last.albedo) = (true, run.albedo);
            }
            continue;
        }
        indices.extend_from_slice(&mesh.indices[range.start as usize..range.end as usize]);
        runs.push(BakeRun {
            end: indices.len() as u32,
            ..run
        });
        previous = Some(range);
    }
    (indices, runs)
}

/// A material's surface: its diffuse albedo, emitted radiance and whether it
/// occludes; white, black and opaque with none, the forward shader's defaults.
/// `end` is left for the caller.
fn surface(
    material: Option<&MaterialAsset>,
    map_average: &dyn Fn(&MapRef) -> Option<Vec3>,
) -> BakeRun {
    let Some(m) = material else {
        return BakeRun {
            end: 0,
            albedo: Vec3::ONE,
            emissive: Vec3::ZERO,
            occludes: true,
        };
    };
    let average = |map: &Option<MapRef>| map.as_ref().and_then(map_average).unwrap_or(Vec3::ONE);
    let albedo = Vec3::from(m.base_color) * average(&m.base_color_map);
    let emissive = Vec3::from(m.emissive) * average(&m.emissive_map);
    let blend = if m.is_transparent() {
        m.alpha.clamp(0.0, 1.0)
    } else {
        1.0
    };
    BakeRun {
        end: 0,
        albedo: albedo.clamp(Vec3::ZERO, Vec3::ONE),
        emissive: emissive * blend,
        occludes: !m.is_transparent(),
    }
}

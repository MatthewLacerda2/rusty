//! What the lightmap bake sees (#438): the static scene flattened to world-space
//! triangles, the lights that take part in the bake, and the sky. Gathered once from a
//! [`Scene`]; the tracer never touches the ECS, so the bake is a pure function of this.

use std::cell::RefCell;

use glam::{Vec2, Vec3};

use crate::asset::MapRef;
use crate::components::mesh::{clip_runs, material_runs};
use crate::components::{LightType, MaterialAsset, MeshComponent, ShadowCasting};
use crate::core::collections::Map;
use crate::scene::Scene;

/// One static mesh in world space. Its occluding runs block and bounce light; the
/// ones with a usable lightmap UV also receive a lightmap, over every drawn run.
/// `cast_shadows` narrows that, as Unity bakes it (#1571): an `Off` mesh is still
/// seen by bounce rays (it bounces and emits) but blocks no light; a `ShadowsOnly`
/// one blocks light but is seen by nothing and gets no lightmap.
#[derive(Clone, Debug, Default)]
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
    /// The run index `index` (into `indices`) belongs to; the last run past the end.
    pub fn run_at(&self, index: u32) -> &BakeRun {
        let k = self.runs.partition_point(|r| r.end <= index);
        &self.runs[k.min(self.runs.len() - 1)]
    }
}

/// Where a baked light sits and how it falls off — the forward shader's models.
#[derive(Clone, Copy, Debug)]
pub enum LightShape {
    /// Light travelling along `direction` (unit).
    Directional {
        direction: Vec3,
    },
    Point {
        position: Vec3,
        range: f32,
    },
    Spot {
        position: Vec3,
        direction: Vec3,
        range: f32,
        cos_inner: f32,
        cos_outer: f32,
    },
}

/// A light the bake includes: a `Mixed` one (bounce only) or a `Baked` one (direct and
/// bounce). `Realtime` lights never get here.
#[derive(Clone, Copy, Debug)]
pub struct BakeLight {
    pub shape: LightShape,
    /// Colour times intensity.
    pub radiance: Vec3,
    /// Whether the receiving texel's own direct light is baked (`Baked`).
    pub bakes_direct: bool,
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
}

impl BakeScene {
    /// Flatten `scene`: the drawn runs of every active static mesh that is not a light
    /// gizmo, each with its own material (#1290) and the mesh's *Cast Shadows*
    /// (#1571), the active `Mixed` / `Baked` lights,
    /// and the ambient sky. `map_average` turns a texture path into its average linear
    /// colour (`None` when it cannot be read); it is asked once per path.
    pub fn gather(scene: &Scene, map_average: &dyn Fn(&MapRef) -> Option<Vec3>) -> Self {
        let map_average = once_per_path(map_average);
        let mut out = BakeScene {
            sky: scene.drawn_ambient().radiance(),
            ..Default::default()
        };
        for id in scene.world.ids_with_mesh() {
            let usable = scene.world.is_active_in_hierarchy(id)
                && scene.world.is_static(id)
                && !scene.world.has_light(id);
            if !usable {
                continue;
            }
            let mesh = scene.world.mesh(id).expect("id came from ids_with_mesh");
            if mesh.is_skinned() {
                continue;
            }
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
            });
        }
        out.gather_lights(scene);
        out
    }

    /// The active lights that take part in the bake (an `Ambient` light is in the
    /// sky already, through `Scene::drawn_ambient`).
    fn gather_lights(&mut self, scene: &Scene) {
        for id in scene.world.ids_with_light() {
            if !scene.world.is_active_in_hierarchy(id) {
                continue;
            }
            let light = scene.world.light(id).expect("id came from ids_with_light");
            // World space: a light on a child bakes where it is drawn (#1196).
            let position = scene.world.world_position(id);
            let forward = scene
                .world
                .transform_direction(id, Vec3::NEG_Z)
                .try_normalize()
                .unwrap_or(Vec3::NEG_Y);
            let shape = match light.light_type {
                // The sky is `Scene::drawn_ambient`'s, which an Ambient light sets.
                LightType::Ambient => continue,
                LightType::Directional => LightShape::Directional { direction: forward },
                LightType::Point => LightShape::Point {
                    position,
                    range: light.range,
                },
                LightType::Spotlight => LightShape::Spot {
                    position,
                    direction: forward,
                    range: light.range,
                    cos_inner: light.inner_cone.to_radians().cos(),
                    cos_outer: light.outer_cone.to_radians().cos(),
                },
            };
            if light.mode.bakes_indirect() {
                self.lights.push(BakeLight {
                    shape,
                    radiance: light.color * light.intensity,
                    bakes_direct: light.mode.bakes_direct(),
                });
            }
        }
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

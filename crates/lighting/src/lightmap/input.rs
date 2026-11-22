use std::cell::RefCell;

use glam::{Vec2, Vec3};

pub use super::lights::{BakeLight, LightShape};
use rusty_asset::MapRef;
use rusty_components::mesh::{clip_runs, material_runs};
use rusty_components::{MaterialAsset, MeshComponent, ShadowCasting};
use rusty_core::collections::Map;
use rusty_scene::Scene;

/// As in Unity, `Off` is seen by bounce rays but blocks nothing; `ShadowsOnly` blocks light and is
/// seen by nothing.
#[derive(Clone, Debug)]
pub struct BakeMesh {
    pub entity: u32,
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub lightmap_uvs: Vec<Vec2>,
    pub indices: Vec<u32>,
    pub runs: Vec<BakeRun>,
    pub cast_shadows: ShadowCasting,
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

#[derive(Clone, Copy, Debug)]
pub struct BakeRun {
    pub end: u32,
    pub albedo: Vec3,
    pub emissive: Vec3,
    /// A Transparent run neither blocks nor bounces but still receives its share of the lightmap.
    pub occludes: bool,
}

impl BakeMesh {
    pub fn receives_lightmap(&self) -> bool {
        self.cast_shadows.is_visible() && self.scale_in_lightmap > 0.0
    }

    pub fn texel_density(&self, edge: u32) -> Option<f32> {
        super::raster::world_per_uv(&self.surface()).map(|w| edge as f32 / w)
    }

    pub fn run_at(&self, index: u32) -> &BakeRun {
        let k = self.runs.partition_point(|r| r.end <= index);
        &self.runs[k.min(self.runs.len() - 1)]
    }
}

/// Colour maps are averaged in linear light; data maps (occlusion) raw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapEncoding {
    Colour,
    Data,
}

#[derive(Clone, Debug, Default)]
pub struct BakeScene {
    pub meshes: Vec<BakeMesh>,
    pub lights: Vec<BakeLight>,
    pub sky: rusty_scene::AmbientRadiance,
    pub diffuse: rusty_scene::lighting::DiffuseModel,
    pub shadowmask: super::lights::MaskChannels,
}

impl BakeScene {
    pub fn gather(scene: &Scene, map_average: &MapAverage<'_>) -> Self {
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
    /// Such a mesh occludes, bounces and emits even without a lightmap (SSIL skips its emission
    /// once a bake exists).
    pub fn takes_mesh(scene: &Scene, id: u32) -> bool {
        rusty_scene::lighting::lightmap_set::bakes_mesh(scene, id)
    }
}

pub type MapAverage<'a> = dyn Fn(&MapRef, MapEncoding) -> Option<Vec3> + 'a;

fn once_per_path<'a>(
    map_average: &'a MapAverage<'a>,
) -> impl Fn(&MapRef, MapEncoding) -> Option<Vec3> + 'a {
    let cache = RefCell::new(Map::default());
    move |map: &MapRef, encoding| {
        let key = (map.clone(), encoding);
        let cached = cache.borrow().get(&key).copied();
        cached.unwrap_or_else(|| {
            *cache
                .borrow_mut()
                .entry(key)
                .or_insert_with(|| map_average(map, encoding))
        })
    }
}

/// A slot past the last run layers over it, judged by its own material.
fn drawn_runs(
    scene: &Scene,
    id: u32,
    mesh: &MeshComponent,
    map_average: &MapAverage<'_>,
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

fn surface(material: Option<&MaterialAsset>, map_average: &MapAverage<'_>) -> BakeRun {
    let Some(m) = material else {
        return BakeRun {
            end: 0,
            albedo: Vec3::ONE,
            emissive: Vec3::ZERO,
            occludes: true,
        };
    };
    let average = |map: &Option<MapRef>, encoding| {
        (map.as_ref())
            .and_then(|map| map_average(map, encoding))
            .unwrap_or(Vec3::ONE)
    };
    let colour = |map: &Option<MapRef>| average(map, MapEncoding::Colour);
    // The forward shader's `mapping::occlusion` lerp on the map's average red, same strength.
    let red = average(&m.occlusion_map, MapEncoding::Data).x;
    let occlusion = 1.0 + m.occlusion_amount() * (red - 1.0);
    let albedo = Vec3::from(m.base_color) * colour(&m.base_color_map) * occlusion;
    let emissive = Vec3::from(m.emissive) * colour(&m.emissive_map);
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

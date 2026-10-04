//! What the lightmap bake sees (#438): the static scene flattened to world-space
//! triangles, the lights that take part in the bake, and the sky. Gathered once from a
//! [`Scene`]; the tracer never touches the ECS, so the bake is a pure function of this.

use glam::{Vec2, Vec3};

use crate::components::{LightType, MaterialAsset};
use crate::scene::Scene;

/// One static mesh in world space. Every one occludes and bounces light; the ones
/// with a usable lightmap UV also receive a lightmap.
#[derive(Clone, Debug, Default)]
pub struct BakeMesh {
    /// The entity the lightmap belongs to.
    pub entity: u32,
    pub positions: Vec<Vec3>,
    /// Unit world-space vertex normals.
    pub normals: Vec<Vec3>,
    pub lightmap_uvs: Vec<Vec2>,
    pub indices: Vec<u32>,
    /// Diffuse reflectance: the material colour times its albedo map's average.
    pub albedo: Vec3,
    /// Emitted radiance, as the forward shader adds it.
    pub emissive: Vec3,
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
    /// The ambient sky colour times intensity: what a ray escaping upward sees. Below
    /// the horizon it fades to a quarter, the forward shader's hemisphere gradient.
    pub sky: Vec3,
}

impl BakeScene {
    /// Flatten `scene`: every active static, opaque mesh that is not a light gizmo, the
    /// active `Mixed` / `Baked` lights, and the ambient sky. `map_average` turns a
    /// texture path into its average linear colour (`None` when it cannot be read).
    pub fn gather(scene: &Scene, map_average: &dyn Fn(&str) -> Option<Vec3>) -> Self {
        let mut out = BakeScene {
            sky: scene.ambient_color * scene.ambient_intensity,
            ..Default::default()
        };
        for id in scene.world.ids_with_mesh() {
            let usable = scene.world.is_active(id)
                && scene.world.is_static(id)
                && !scene.world.has_light(id);
            let material = scene.material_asset_of(id);
            if !usable || material.is_some_and(MaterialAsset::is_transparent) {
                continue;
            }
            let mesh = scene.world.mesh(id).expect("id came from ids_with_mesh");
            if mesh.is_skinned() || mesh.indices.is_empty() {
                continue;
            }
            let matrix = scene.world_matrix(id);
            let normal_matrix = matrix.inverse().transpose();
            let (albedo, emissive) = surface(material, map_average);
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
                indices: mesh.indices.clone(),
                albedo,
                emissive,
            });
        }
        out.gather_lights(scene);
        out
    }

    /// The active lights that take part in the bake, plus an `Ambient` light's sky
    /// (it replaces the scene ambient, as it does in the renderer).
    fn gather_lights(&mut self, scene: &Scene) {
        for id in scene.world.ids_with_light() {
            if !scene.world.is_active(id) {
                continue;
            }
            let light = scene.world.light(id).expect("id came from ids_with_light");
            let t = scene.world.transform(id).expect("mandatory Transform");
            let forward = (t.rotation * Vec3::NEG_Z)
                .try_normalize()
                .unwrap_or(Vec3::NEG_Y);
            let shape = match light.light_type {
                LightType::Ambient => {
                    self.sky = light.color * light.intensity;
                    continue;
                }
                LightType::Directional => LightShape::Directional { direction: forward },
                LightType::Point => LightShape::Point {
                    position: t.position,
                    range: light.range,
                },
                LightType::Spotlight => LightShape::Spot {
                    position: t.position,
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

/// A material's diffuse albedo and emitted radiance; white and black with none, the
/// forward shader's defaults.
fn surface(
    material: Option<&MaterialAsset>,
    map_average: &dyn Fn(&str) -> Option<Vec3>,
) -> (Vec3, Vec3) {
    let Some(m) = material else {
        return (Vec3::ONE, Vec3::ZERO);
    };
    let average = |map: &Option<String>| map.as_deref().and_then(map_average).unwrap_or(Vec3::ONE);
    let albedo = Vec3::from(m.base_color) * average(&m.base_color_map);
    let emissive = Vec3::from(m.emissive) * average(&m.emissive_map);
    (albedo.clamp(Vec3::ZERO, Vec3::ONE), emissive)
}

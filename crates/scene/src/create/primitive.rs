use glam::Vec3;

use crate::create::defaults::{default_collider, light};
use crate::rehydrate::bounds_of;
use crate::{LightComponent, LightType, MeshComponent};
use rusty_asset::mesh_data::Shared;
use rusty_components::mesh::primitives;
use rusty_components::mesh::{MeshRevision, Vertex};
use rusty_components::{CapsuleAxis, ColliderComponent, ColliderShape};

// The round primitives' radius and the Cylinder's and Capsule's full height, in metres
// (Unity's): one value for the mesh and the collider it comes with.
const RADIUS: f32 = 0.5;
const HEIGHT: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primitive {
    Box,
    Sphere,
    Plane,
    Cylinder,
    Capsule,
    PointLight,
    DirectionalLight,
    SpotLight,
}

impl Primitive {
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "box" => Some(Self::Box),
            "sphere" => Some(Self::Sphere),
            "plane" => Some(Self::Plane),
            "cylinder" => Some(Self::Cylinder),
            "capsule" => Some(Self::Capsule),
            "pointlight" => Some(Self::PointLight),
            "directionallight" => Some(Self::DirectionalLight),
            "spotlight" => Some(Self::SpotLight),
            _ => None,
        }
    }
}

pub fn primitive_geometry(primitive: Primitive) -> Option<Geometry> {
    subdivided_geometry(primitive, 1)
}

pub type Geometry = (Shared<Vertex>, Shared<u32>);

/// Built once per process per shape and shared; a finer Plane cut is built afresh, so a
/// cut tried once is not kept. A load rebuilds primitives from this too.
pub fn subdivided_geometry(primitive: Primitive, subdivisions: u32) -> Option<Geometry> {
    if primitive == Primitive::Plane && subdivisions != 1 {
        return generated(primitive, subdivisions).map(|(v, i)| (v.into(), i.into()));
    }
    type Built = Option<Geometry>;
    static BUILT: std::sync::Mutex<Vec<(Primitive, Built)>> = std::sync::Mutex::new(Vec::new());
    let mut built = BUILT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, geometry)) = built.iter().find(|(p, _)| *p == primitive) {
        return geometry.clone();
    }
    let geometry = generated(primitive, 1).map(|(v, i)| (v.into(), i.into()));
    built.push((primitive, geometry.clone()));
    geometry
}

/// One revision per shape and cut, so a renderer uploads a primitive once.
pub fn primitive_revision(primitive: Primitive, subdivisions: u32) -> MeshRevision {
    let cut = if primitive == Primitive::Plane {
        subdivisions
    } else {
        1
    };
    static GIVEN: std::sync::Mutex<Vec<((Primitive, u32), MeshRevision)>> =
        std::sync::Mutex::new(Vec::new());
    let mut given = GIVEN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, revision)) = given.iter().find(|(key, _)| *key == (primitive, cut)) {
        return *revision;
    }
    let revision = MeshRevision::next();
    given.push(((primitive, cut), revision));
    revision
}

fn generated(primitive: Primitive, subdivisions: u32) -> Option<(Vec<Vertex>, Vec<u32>)> {
    match primitive {
        Primitive::Box => Some(primitives::generate_box(1.0, 1.0, 1.0)),
        Primitive::Sphere => Some(primitives::generate_sphere(RADIUS, 16, 16)),
        // 15 m a side.
        Primitive::Plane => Some(primitives::generate_plane_grid(15.0, 15.0, subdivisions)),
        // Unity's Cylinder: diameter 1, height 2, along Y.
        Primitive::Cylinder => Some(primitives::generate_cylinder(
            Vec3::new(0.0, -HEIGHT / 2.0, 0.0),
            Vec3::new(0.0, HEIGHT / 2.0, 0.0),
            RADIUS,
            12,
        )),
        // Unity's Capsule: radius 0.5, full height 2, along Y.
        Primitive::Capsule => Some(primitives::generate_capsule(RADIUS, HEIGHT, 16, 8)),
        Primitive::PointLight | Primitive::DirectionalLight | Primitive::SpotLight => None,
    }
}

fn primitive_mesh_name(primitive: Primitive) -> Option<&'static str> {
    match primitive {
        Primitive::Box => Some("Box"),
        Primitive::Sphere => Some("Sphere"),
        Primitive::Plane => Some("Plane"),
        Primitive::Cylinder => Some("Cylinder"),
        Primitive::Capsule => Some("Capsule"),
        _ => None,
    }
}

pub fn primitive_mesh_component(primitive: Primitive) -> Option<MeshComponent> {
    let name = primitive_mesh_name(primitive)?;
    let (vertices, indices) = primitive_geometry(primitive)?;
    let mut mesh = MeshComponent::new(name, vertices, indices);
    mesh.revision = primitive_revision(primitive, 1);
    Some(mesh)
}

pub(super) fn primitive_light(primitive: Primitive) -> Option<LightComponent> {
    match primitive {
        Primitive::PointLight => Some(light(LightType::Point, Vec3::ONE, 1.5, 10.0)),
        Primitive::DirectionalLight => Some(light(
            LightType::Directional,
            Vec3::new(1.0, 0.95, 0.8),
            2.0,
            0.0,
        )),
        Primitive::SpotLight => Some(light(LightType::Spotlight, Vec3::ONE, 2.0, 15.0)),
        _ => None,
    }
}

/// As Unity's `CreatePrimitive`, a Cylinder gets a capsule collider.
pub fn primitive_collider(primitive: Primitive, mesh: &MeshComponent) -> Option<ColliderComponent> {
    let shape = match primitive {
        Primitive::Box => ColliderShape::Box { size: Vec3::ONE },
        Primitive::Sphere => ColliderShape::Sphere { radius: RADIUS },
        Primitive::Capsule | Primitive::Cylinder => ColliderShape::Capsule {
            radius: RADIUS,
            height: HEIGHT,
            axis: CapsuleAxis::Y,
        },
        Primitive::Plane => {
            let (lo, hi) = bounds_of(mesh.vertices.iter().map(|v| v.position))?;
            return Some(ColliderComponent::from_mesh_bounds(false, lo, hi));
        }
        Primitive::PointLight | Primitive::DirectionalLight | Primitive::SpotLight => return None,
    };
    Some(ColliderComponent {
        shape,
        ..default_collider()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_primitive_is_what_building_it_again_gives() {
        for p in [
            Primitive::Box,
            Primitive::Sphere,
            Primitive::Cylinder,
            Primitive::Capsule,
        ] {
            let fresh = generated(p, 1).expect("a mesh primitive has geometry");
            for _ in 0..2 {
                let kept = subdivided_geometry(p, 7).expect("and keeps it");
                assert!(kept.0[..] == fresh.0[..] && kept.1 == fresh.1, "{p:?}");
            }
        }
        assert!(subdivided_geometry(Primitive::SpotLight, 1).is_none());
        let plane = |n| subdivided_geometry(Primitive::Plane, n).unwrap().1.len();
        assert!(
            plane(4) > plane(1),
            "a Plane is still cut by its subdivisions"
        );
    }

    #[test]
    fn two_entities_of_one_primitive_share_its_buffers() {
        for p in [Primitive::Sphere, Primitive::Plane] {
            let mut scene = crate::Scene::new();
            let [a, b] = [0, 1].map(|_| crate::create::create_entity(&mut scene, "P", Some(p)));
            let (a, b) = (scene.world.mesh(a).unwrap(), scene.world.mesh(b).unwrap());
            assert!(Shared::same(&a.vertices, &b.vertices), "{p:?}");
            assert!(Shared::same(&a.indices, &b.indices), "{p:?}");
        }
    }
}

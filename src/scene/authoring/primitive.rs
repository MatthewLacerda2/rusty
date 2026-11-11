//! src/scene/authoring/primitive.rs — The Create menu's primitives (#2051).
//!
//! What each primitive the hierarchy's Create menu and `Scene.CreateEntity` offer
//! comes with, as Unity's `GameObject.CreatePrimitive`: a mesh primitive gets its
//! mesh and the collider matching it, a light primitive its light. The geometry
//! here is also what serialization's rehydrate path rebuilds a saved primitive from.

use glam::Vec3;

use crate::components::mesh::primitives;
use crate::components::mesh::Vertex;
use crate::components::{CapsuleAxis, ColliderComponent, ColliderShape};
use crate::scene::authoring::defaults::{default_collider, light};
use crate::scene::rehydrate::bounds_of;
use crate::scene::{LightComponent, LightType, MeshComponent};

/// The round primitives' radius and the Cylinder's and Capsule's full height, metres
/// (Unity's): one value for the mesh and the collider it comes with.
const RADIUS: f32 = 0.5;
const HEIGHT: f32 = 2.0;

/// The primitive set the hierarchy toolbar's "Create" dropdown offers — meshes
/// and lights. A `None` primitive (or an unrecognised name) creates a bare entity
/// carrying only its mandatory `Transform`, matching the toolbar's behaviour for
/// a name with no primitive picked.
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
    /// Parse a toolbar/API primitive name (case-insensitive). Returns `None` for
    /// an empty/unknown name, so the caller creates a bare entity.
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

/// The geometry for a primitive mesh (`Box`/`Sphere`/`Plane`/`Cylinder`/`Capsule`). This is
/// the single source of truth for primitive shapes, reused by serialization's
/// `rehydrate_one` so a created primitive and a loaded one are byte-identical.
/// Returns `None` for the light primitives, which carry no mesh.
pub fn primitive_geometry(primitive: Primitive) -> Option<(Vec<Vertex>, Vec<u32>)> {
    subdivided_geometry(primitive, 1)
}

/// [`primitive_geometry`], a `Plane` cut into `subdivisions` quads per side (#1524,
/// `MeshComponent::subdivisions`, clamped to its range); every other shape ignores it.
pub fn subdivided_geometry(
    primitive: Primitive,
    subdivisions: u32,
) -> Option<(Vec<Vertex>, Vec<u32>)> {
    match primitive {
        Primitive::Box => Some(primitives::generate_box(1.0, 1.0, 1.0)),
        Primitive::Sphere => Some(primitives::generate_sphere(RADIUS, 16, 16)),
        Primitive::Plane => Some(primitives::generate_plane_grid(15.0, 15.0, subdivisions)),
        // Unity's Cylinder: diameter 1, height 2, along Y (#1621).
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

/// The canonical primitive-mesh name (matches `MeshComponent::primitive_type` and
/// the `rehydrate_one` keys), or `None` for the light primitives.
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

/// Build the `MeshComponent` for a mesh primitive (geometry + the on-disk
/// `primitive_type` reference). Returns `None` for the light primitives.
pub fn primitive_mesh_component(primitive: Primitive) -> Option<MeshComponent> {
    let name = primitive_mesh_name(primitive)?;
    let (vertices, indices) = primitive_geometry(primitive)?;
    Some(MeshComponent::new(name, vertices, indices))
}

/// The default light for a light primitive (the hierarchy toolbar's values).
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

/// The collider a mesh primitive comes with, sized to `mesh` (its own geometry), as
/// Unity's `CreatePrimitive` gives one: Box a 1 m box, Sphere a 0.5 m sphere,
/// Capsule and Cylinder a 0.5 m × 2 m capsule along Y (Unity gives a cylinder a
/// capsule too), and Plane a triangle-mesh collider on its own mesh. `None` for the
/// light primitives.
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

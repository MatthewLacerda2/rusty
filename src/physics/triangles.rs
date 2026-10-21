//! src/physics/triangles.rs — a collider's world-space triangles, for the navmesh bake.
//!
//! The navmesh (#454) rasterises the geometry physics actually collides with, so it
//! builds the same parry shape [`build_shape`] gives rapier (world scale baked in,
//! Unity-style) and tessellates it at the entity's world pose. Boxes, hulls and
//! trimeshes come out exact; spheres, cylinders and capsules are tessellated finely
//! enough for a grid whose cells are tens of centimetres wide.

use glam::Vec3;
use rapier3d::prelude::*;

use super::build::{build_shape_for, collider_inputs};
use super::compound::world_pose;
use crate::scene::Scene;

/// Segments around a round shape's circumference.
const ROUND_SUBDIV: u32 = 16;
/// Segments from pole to pole of a sphere / capsule cap.
const POLE_SUBDIV: u32 = 8;

/// One collider's surface as world-space triangles.
pub struct ColliderTriangles {
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
    /// The shape is convex (everything but a non-convex mesh collider). A vertical
    /// line crosses a convex solid in one interval, which lets the bake fill the
    /// solid between its lowest and highest triangle instead of trusting winding.
    pub convex: bool,
}

/// `id`'s active collider as world-space triangles, or `None` when it has no active
/// collider or a degenerate mesh. Pure geometry: deterministic for a given scene.
pub fn collider_world_triangles(scene: &Scene, id: u32) -> Option<ColliderTriangles> {
    let inp = collider_inputs(&scene.world, id)?;
    let pose = world_pose(scene, id)?;
    let mesh = inp
        .mesh_geom
        .as_ref()
        .map(|(p, i)| (p.as_slice(), i.as_slice()));
    let collider = build_shape_for(id, &inp.shape, pose.scale, mesh)?;
    let (points, triangles, convex) = tessellate(collider.shape())?;
    let vertices = points
        .into_iter()
        .map(|p| pose.pos + pose.rot * p)
        .collect();
    Some(ColliderTriangles {
        vertices,
        triangles,
        convex,
    })
}

type Tessellation = (Vec<Vector>, Vec<[u32; 3]>, bool);

/// The local-space triangles of every shape `build_shape` produces.
fn tessellate(shape: &dyn Shape) -> Option<Tessellation> {
    if let Some(s) = shape.as_cuboid() {
        let (v, i) = s.to_trimesh();
        return Some((v, i, true));
    }
    if let Some(s) = shape.as_ball() {
        let (v, i) = s.to_trimesh(ROUND_SUBDIV, POLE_SUBDIV);
        return Some((v, i, true));
    }
    if let Some(s) = shape.as_cylinder() {
        let (v, i) = s.to_trimesh(ROUND_SUBDIV);
        return Some((v, i, true));
    }
    if let Some(s) = shape.as_capsule() {
        let (v, i) = s.to_trimesh(ROUND_SUBDIV, POLE_SUBDIV);
        return Some((v, i, true));
    }
    if let Some(s) = shape.as_convex_polyhedron() {
        let (v, i) = s.to_trimesh();
        return Some((v, i, true));
    }
    let mesh = shape.as_trimesh()?;
    Some((mesh.vertices().to_vec(), mesh.indices().to_vec(), false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::build::build_shape;
    use crate::scene::{ColliderComponent, ColliderShape};
    use glam::Quat;

    fn boxed(scene: &mut Scene, size: Vec3, pos: Vec3, rot: Quat) -> u32 {
        let id = scene.add_entity("box".to_string());
        {
            let mut t = scene.world.transform_mut(id).expect("fresh entity");
            t.position = pos;
            t.rotation = rot;
        }
        let mut col = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
        col.shape = ColliderShape::Box { size };
        scene.world.set_collider(id, Some(col));
        id
    }

    #[test]
    fn box_comes_out_as_twelve_world_triangles() {
        let mut scene = Scene::new();
        let id = boxed(
            &mut scene,
            Vec3::new(2.0, 1.0, 4.0),
            Vec3::new(5.0, 0.5, 0.0),
            Quat::IDENTITY,
        );
        let tri = collider_world_triangles(&scene, id).expect("a box tessellates");
        assert!(tri.convex);
        assert_eq!(tri.triangles.len(), 12);
        let min = tri.vertices.iter().fold(Vec3::MAX, |a, &v| a.min(v));
        let max = tri.vertices.iter().fold(Vec3::MIN, |a, &v| a.max(v));
        assert!(min.abs_diff_eq(Vec3::new(4.0, 0.0, -2.0), 1e-5), "{min}");
        assert!(max.abs_diff_eq(Vec3::new(6.0, 1.0, 2.0), 1e-5), "{max}");
    }

    #[test]
    fn rotation_tilts_the_box_into_a_ramp() {
        let mut scene = Scene::new();
        let tilt = Quat::from_rotation_z(0.3);
        let id = boxed(&mut scene, Vec3::new(4.0, 0.2, 2.0), Vec3::ZERO, tilt);
        let tri = collider_world_triangles(&scene, id).expect("a box tessellates");
        let top = tri.vertices.iter().map(|v| v.y).fold(f32::MIN, f32::max);
        assert!(
            top > 0.5,
            "a tilted box reaches above its flat half-height: {top}"
        );
    }

    /// Off the origin and turned, every triangle still faces out of the box: the
    /// pose moves each vertex, it never mirrors the shape inside out.
    #[test]
    fn an_off_origin_turned_box_keeps_its_triangles_facing_out() {
        let mut scene = Scene::new();
        let center = Vec3::new(5.0, 1.0, -3.0);
        let turn = Quat::from_rotation_y(0.7) * Quat::from_rotation_x(0.2);
        let id = boxed(&mut scene, Vec3::new(2.0, 1.0, 4.0), center, turn);
        let tri = collider_world_triangles(&scene, id).expect("a box tessellates");
        for [a, b, c] in tri.triangles {
            let [a, b, c] = [a, b, c].map(|i| tri.vertices[i as usize]);
            let normal = (b - a).cross(c - a);
            let outward = (a + b + c) / 3.0 - center;
            assert!(normal.dot(outward) > 0.0, "inward face {a} {b} {c}");
        }
    }

    /// Every shape `build_shape` makes tessellates: round ones finely enough to
    /// reach their radius, a hull as convex, a trimesh as itself and not convex.
    #[test]
    fn every_built_shape_tessellates_within_its_extent() {
        use crate::components::CapsuleAxis;
        let round = [
            ColliderShape::Sphere { radius: 1.0 },
            ColliderShape::Cylinder {
                radius: 1.0,
                height: 2.0,
            },
            ColliderShape::Capsule {
                radius: 1.0,
                height: 4.0,
                axis: CapsuleAxis::Y,
            },
        ];
        for shape in round {
            let built = build_shape(&shape, Vec3::ONE, None).expect("analytic shape");
            let (v, tris, convex) = tessellate(built.shape()).expect("tessellates");
            assert!(
                convex && tris.len() >= 16,
                "{shape:?}: {} triangles",
                tris.len()
            );
            let reach = v.iter().map(|p| p.x.abs()).fold(0.0, f32::max);
            assert!(
                (reach - 1.0).abs() < 1e-4,
                "{shape:?} reaches its radius: {reach}"
            );
        }
        let tetra = vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let faces = [0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3];
        for convex in [true, false] {
            let shape = ColliderShape::Mesh {
                convex,
                local_min: Vec3::ZERO,
                local_max: Vec3::ONE,
            };
            let built = build_shape(&shape, Vec3::ONE, Some((&tetra, &faces))).expect("mesh");
            let (v, tris, is_convex) = tessellate(built.shape()).expect("tessellates");
            assert_eq!((v.len(), tris.len(), is_convex), (4, 4, convex));
        }
    }
}

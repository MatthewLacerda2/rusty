//! src/physics/triangles.rs — a collider's world-space triangles, for the navmesh bake.
//!
//! The navmesh (#454) rasterises the geometry physics actually collides with, so it
//! builds the same parry shape [`build_shape`] gives rapier (world scale baked in,
//! Unity-style) and tessellates it at the entity's world pose. Boxes, hulls and
//! trimeshes come out exact; spheres, cylinders and capsules are tessellated finely
//! enough for a grid whose cells are tens of centimetres wide.

use glam::Vec3;
use rapier3d::prelude::*;

use super::build::{build_shape, collider_inputs};
use super::compound::world_pose;
use super::convert::from_na_point;
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
    let collider = build_shape(&inp.shape, pose.scale, mesh)?;
    let (points, triangles, convex) = tessellate(collider.shape())?;
    let vertices = points
        .into_iter()
        .map(|p| pose.pos + pose.rot * from_na_point(p))
        .collect();
    Some(ColliderTriangles {
        vertices,
        triangles,
        convex,
    })
}

type Tessellation = (Vec<Point<f32>>, Vec<[u32; 3]>, bool);

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
}

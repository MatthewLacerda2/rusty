//! What the occlusion bake reads (#1714), and what the renderer checks it against:
//! which static meshes take part, each one's [`fingerprint`], and the occluders'
//! world-space triangles with the side of each a camera sees.

use std::collections::BTreeSet;

use glam::{Mat4, Vec3};

use crate::components::mesh::{clip_runs, material_runs};
use crate::components::{MaterialAsset, MeshComponent, RenderFace, RenderMode};
use crate::scene::Scene;

/// Which side of an occluder's triangle stops a view: the one its world winding
/// faces (`Ccw`), the other (`Cw`), or both. A mirrored mesh's front faces wind the
/// other way, as the renderer's `Facing` says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Face {
    Ccw,
    Cw,
    Both,
}

impl Face {
    fn of(world: Mat4, face: RenderFace) -> Self {
        let mirrored = world.determinant() < 0.0;
        match (face, mirrored) {
            (RenderFace::Both, _) => Self::Both,
            (RenderFace::Front, false) | (RenderFace::Back, true) => Self::Ccw,
            (RenderFace::Front, true) | (RenderFace::Back, false) => Self::Cw,
        }
    }

    /// Whether a ray along `dir` meets the drawn side of a triangle whose world
    /// winding normal is `normal`.
    pub(super) fn stops(self, normal: Vec3, dir: Vec3) -> bool {
        match self {
            Self::Ccw => normal.dot(dir) < 0.0,
            Self::Cw => normal.dot(dir) > 0.0,
            Self::Both => true,
        }
    }
}

/// One object as the bake sees it: its drawn triangles (where a view must reach),
/// and its occluding ones with the side each draws (empty for an occludee alone).
pub(super) struct Object {
    pub entity: u32,
    pub fingerprint: u64,
    pub drawn: Vec<[Vec3; 3]>,
    pub occluding: Vec<([Vec3; 3], Face)>,
}

/// The scene-wide facts a fingerprint reads, gathered once: the renderers every
/// `LODGroup` holds (which never occlude: the drawn level moves with the camera).
pub struct Fingerprints {
    grouped: BTreeSet<u32>,
}

impl Fingerprints {
    pub fn of_scene(scene: &Scene) -> Self {
        let world = &scene.world;
        let groups = world.ids_with_lod_group().into_iter();
        let grouped = groups
            .filter_map(|id| {
                world
                    .lod_group(id)
                    .map(|g| g.renderers().collect::<Vec<_>>())
            })
            .flatten()
            .collect();
        Self { grouped }
    }

    /// Entity `id`'s fingerprint and whether it occludes, `None` when it takes no
    /// part in the bake: not a mesh, inactive, not static, skinned (its posed bounds
    /// move), shadows-only (no camera draws it) or drawing no triangle.
    pub fn of(&self, scene: &Scene, id: u32) -> Option<(u64, bool)> {
        let world = &scene.world;
        let mesh = world.mesh(id)?;
        let takes_part = world.is_active_in_hierarchy(id)
            && world.is_static(id)
            && !mesh.is_skinned()
            && mesh.active_palette().is_empty()
            && mesh.cast_shadows.is_visible()
            && !mesh.indices.is_empty();
        if !takes_part {
            return None;
        }
        let slots = slot_materials(scene, id, &mesh);
        let may_occlude =
            !mesh.culls_when_small() && !self.grouped.contains(&id) && !world.has_light(id);
        let occluder = may_occlude && slots.iter().any(|(_, m)| occludes(*m));
        let mut bytes = Vec::with_capacity(256);
        word(&mut bytes, mesh.primitive_type.as_bytes());
        word(
            &mut bytes,
            mesh.asset_ref.as_deref().unwrap_or("").as_bytes(),
        );
        let counts = [
            mesh.subdivisions,
            mesh.vertices.len() as u32,
            mesh.indices.len() as u32,
            u32::from(world.layer(id)),
            u32::from(occluder),
        ];
        counts.iter().for_each(|c| bytes.extend(c.to_le_bytes()));
        for range in &mesh.ranges {
            bytes.extend(range.start.to_le_bytes());
            bytes.extend(range.end.to_le_bytes());
        }
        let matrix = world.local_to_world(id).to_cols_array();
        matrix
            .iter()
            .for_each(|f| bytes.extend(f.to_bits().to_le_bytes()));
        for (name, material) in &slots {
            word(&mut bytes, name.as_bytes());
            let face = material.map_or(RenderFace::Front, |m| m.render_face);
            bytes.extend([u8::from(occludes(*material)), face as u8]);
        }
        Some((crate::core::hash::content(&bytes), occluder))
    }

    /// Entity `id` ready to bake, `None` when it takes no part.
    pub(super) fn object(&self, scene: &Scene, id: u32) -> Option<Object> {
        let (fingerprint, occluder) = self.of(scene, id)?;
        let mesh = scene.world.mesh(id)?;
        let world = scene.world.local_to_world(id);
        let corner =
            |i: u32| world.transform_point3(Vec3::from(mesh.vertices[i as usize].position));
        let ranges = clip_runs(mesh.index_ranges(), mesh.indices.len() as u32);
        let count = scene.world.material(id).map(|m| m.materials.len());
        let (mut drawn, mut occluding) = (Vec::new(), Vec::new());
        for (slot, range) in material_runs(ranges, count) {
            let material = count.and_then(|_| scene.material_entry_at(id, slot).map(|(_, m)| m));
            let face = Face::of(world, material.map_or(RenderFace::Front, |m| m.render_face));
            for tri in mesh.indices[range.start as usize..range.end as usize].chunks_exact(3) {
                let tri = [corner(tri[0]), corner(tri[1]), corner(tri[2])];
                drawn.push(tri);
                if occluder && occludes(material) {
                    occluding.push((tri, face));
                }
            }
        }
        Some(Object {
            entity: id,
            fingerprint,
            drawn,
            occluding,
        })
    }
}

/// Entity `id`'s fingerprint, gathering the scene-wide facts for it alone: what
/// the bake stores for it and the renderer compares ([`Fingerprints::of`]).
pub fn fingerprint(scene: &Scene, id: u32) -> Option<u64> {
    Fingerprints::of_scene(scene).of(scene, id).map(|(f, _)| f)
}

/// Whether a slot drawn with `material` stops a view: opaque, with no cut and no
/// surface shader (whose vertex stage may move what it draws). No material draws
/// the engine's default, which is opaque.
fn occludes(material: Option<&MaterialAsset>) -> bool {
    material.is_none_or(|m| m.render_mode == RenderMode::Opaque && m.shader.is_none())
}

/// Entity `id`'s material slots by name, `("", None)` for a slot it draws with no
/// material.
fn slot_materials<'a>(
    scene: &'a Scene,
    id: u32,
    mesh: &MeshComponent,
) -> Vec<(String, Option<&'a MaterialAsset>)> {
    let ranges = clip_runs(mesh.index_ranges(), mesh.indices.len() as u32);
    let count = scene.world.material(id).map(|m| m.materials.len());
    let slots = material_runs(ranges, count)
        .into_iter()
        .map(|(slot, _)| slot);
    let entry = |slot| count.and_then(|_| scene.material_entry_at(id, slot));
    slots
        .map(|slot| match entry(slot) {
            Some((name, m)) => (name.to_string(), Some(m)),
            None => (String::new(), None),
        })
        .collect()
}

/// `text` as a length-prefixed word of the fingerprint.
fn word(bytes: &mut Vec<u8>, text: &[u8]) {
    bytes.extend((text.len() as u32).to_le_bytes());
    bytes.extend(text);
}

//! What the occlusion bake reads (#1714), and what the renderer checks it against:
//! which static meshes take part, each one's [`fingerprint`], and the occluders'
//! world-space triangles with the side of each a camera sees.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use glam::{Mat4, Vec3};

use crate::components::mesh::{clip_runs, material_runs, MeshRevision};
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

    /// The normal of the side a camera sees, given the world winding normal
    /// `normal`; `None` when both sides draw.
    pub(super) fn front(self, normal: Vec3) -> Option<Vec3> {
        match self {
            Self::Ccw => Some(normal),
            Self::Cw => Some(-normal),
            Self::Both => None,
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

/// One object as the bake sees it: its drawn triangles (where a view must reach)
/// and its occluding ones (empty for an occludee alone), each with the side it draws.
pub(super) struct Object {
    pub entity: u32,
    pub fingerprint: u64,
    pub drawn: Vec<([Vec3; 3], Face)>,
    pub occluding: Vec<([Vec3; 3], Face)>,
}

/// The scene-wide facts a fingerprint reads, gathered once: the renderers every
/// `LODGroup` holds (which never occlude: the drawn level moves with the camera),
/// and each geometry's content hash, kept by its revision so a re-check of an
/// unchanged level hashes no vertex.
#[derive(Default)]
pub struct Fingerprints {
    grouped: BTreeSet<u32>,
    /// Revision → (its positions' and indices' hash, the sweep it was last used in).
    /// The revision is only the cache's key: the hash is of the vertices themselves,
    /// so a fingerprint is the same whatever revision a geometry carries.
    geometry: RefCell<BTreeMap<MeshRevision, (u64, u64)>>,
    sweep: u64,
}

impl Fingerprints {
    pub fn of_scene(scene: &Scene) -> Self {
        Self {
            grouped: grouped(scene),
            ..Self::default()
        }
    }

    /// Re-read the `LODGroup`s: whether which renderers they hold changed, which
    /// changes those renderers' fingerprints.
    pub fn regroup(&mut self, scene: &Scene) -> bool {
        let now = grouped(scene);
        let changed = now != self.grouped;
        self.grouped = now;
        changed
    }

    /// Start a sweep over every baked object: geometry hashes no object of the
    /// last sweep read are dropped at the next one, so the cache never outgrows
    /// the geometries in use.
    pub fn sweep(&mut self) {
        let last = self.sweep;
        self.geometry.get_mut().retain(|_, (_, used)| *used == last);
        self.sweep += 1;
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
            && mesh.cast_shadows.is_visible();
        if !takes_part || drawn_runs(scene, id, &mesh).is_empty() {
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
        bytes.extend(self.geometry(&mesh).to_le_bytes());
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
        let count = scene.world.material(id).map(|m| m.materials.len());
        let (mut drawn, mut occluding) = (Vec::new(), Vec::new());
        for (slot, range) in drawn_runs(scene, id, &mesh) {
            let material = count.and_then(|_| scene.material_entry_at(id, slot).map(|(_, m)| m));
            let face = Face::of(world, material.map_or(RenderFace::Front, |m| m.render_face));
            for tri in mesh.indices[range.start as usize..range.end as usize].chunks_exact(3) {
                let tri = [corner(tri[0]), corner(tri[1]), corner(tri[2])];
                drawn.push((tri, face));
                if occluder && occludes(material) {
                    occluding.push((tri, face));
                }
            }
        }
        // A run shorter than a triangle draws nothing: such an object takes no part.
        if drawn.is_empty() {
            return None;
        }
        Some(Object {
            entity: id,
            fingerprint,
            drawn,
            occluding,
        })
    }
}

impl Fingerprints {
    /// `mesh`'s positions and indices as one hash, computed once per revision.
    fn geometry(&self, mesh: &MeshComponent) -> u64 {
        let mut cache = self.geometry.borrow_mut();
        let entry = cache.entry(mesh.revision).or_insert_with(|| {
            let mut bytes = Vec::with_capacity(mesh.vertices.len() * 12 + mesh.indices.len() * 4);
            for v in &mesh.vertices {
                v.position
                    .iter()
                    .for_each(|f| bytes.extend(f.to_bits().to_le_bytes()));
            }
            mesh.indices
                .iter()
                .for_each(|i| bytes.extend(i.to_le_bytes()));
            (crate::core::hash::content(&bytes), self.sweep)
        });
        entry.1 = self.sweep;
        entry.0
    }
}

/// The renderers every `LODGroup` of `scene` holds.
fn grouped(scene: &Scene) -> BTreeSet<u32> {
    let world = &scene.world;
    let groups = world.ids_with_lod_group().into_iter();
    let renderers = groups.filter_map(|id| {
        world
            .lod_group(id)
            .map(|g| g.renderers().collect::<Vec<_>>())
    });
    renderers.flatten().collect()
}

/// Entity `id`'s index runs by material slot, each holding at least one whole
/// triangle: empty when it draws nothing (no index, or an empty material list).
fn drawn_runs(scene: &Scene, id: u32, mesh: &MeshComponent) -> Vec<(usize, std::ops::Range<u32>)> {
    let ranges = clip_runs(mesh.index_ranges(), mesh.indices.len() as u32);
    let count = scene.world.material(id).map(|m| m.materials.len());
    let mut runs = material_runs(ranges, count);
    runs.retain(|(_, r)| r.len() >= 3);
    runs
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
    let count = scene.world.material(id).map(|m| m.materials.len());
    let slots = drawn_runs(scene, id, mesh)
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

//! What the occlusion bake reads (#1714), and what the renderer checks it against:
//! which static meshes take part, each one's [`fingerprint`], and the occluders'
//! world-space triangles with the side of each a camera sees.
//!
//! stored-format: the fingerprint each baked object carries in the `<scene>.occlusion` file

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use glam::{Mat4, Vec3};

use rusty_components::mesh::{clip_runs, material_runs, MeshRevision};
use rusty_components::{MaterialAsset, MeshComponent, RenderFace, RenderMode};
use rusty_scene::Scene;

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
/// and each geometry's `corners_hash`, kept by its revision so a re-check of an
/// unchanged level hashes no vertex.
#[derive(Default)]
pub struct Fingerprints {
    grouped: BTreeSet<u32>,
    /// Revision → (its [`corners_hash`], the sweep it was last used in). The
    /// revision is only the cache's key: the hash is of the triangles' corner
    /// positions, read through the indices in index order, never of the vertex
    /// buffer as stored. So a geometry re-imported under a new revision, or with
    /// its corners welded or split another way (normals, UV seams, a re-export),
    /// keeps its fingerprint, and only a moved corner or a changed triangle breaks
    /// it (#1958). Hashing the raw buffers instead read a re-weld as a change.
    geometry: RefCell<BTreeMap<MeshRevision, (u64, u64)>>,
    sweep: u64,
}

/// A run of a mesh's indices drawn with one material slot.
type Run = (usize, std::ops::Range<u32>);

/// What [`Fingerprints::read`] found of an entity that takes part: its print,
/// whether it occludes, its drawn runs and each run's material.
struct Read<'a> {
    fingerprint: u64,
    occluder: bool,
    runs: Vec<Run>,
    materials: Vec<Option<&'a MaterialAsset>>,
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
    /// part in the bake (`takes_part`) or draws no triangle.
    pub fn of(&self, scene: &Scene, id: u32) -> Option<(u64, bool)> {
        let mesh = scene.world.mesh(id)?;
        let read = self.read(scene, id, &mesh)?;
        Some((read.fingerprint, read.occluder))
    }

    /// [`Self::of`], keeping the runs and materials it looked up.
    fn read<'a>(&self, scene: &'a Scene, id: u32, mesh: &MeshComponent) -> Option<Read<'a>> {
        let world = &scene.world;
        if !takes_part(scene, id, mesh) {
            return None;
        }
        let runs = drawn_runs(scene, id, mesh);
        if runs.is_empty() {
            return None;
        }
        let materials: Vec<_> = runs
            .iter()
            .map(|(slot, _)| slot_material(scene, id, *slot))
            .collect();
        let may_occlude =
            !mesh.culls_when_small() && !self.grouped.contains(&id) && !world.has_light(id);
        let occluder = may_occlude && materials.iter().any(|m| occludes(*m));
        // Only what the bake reads (#1958): the triangles, not how the vertex buffer
        // stores them, and each drawn run's side and whether it stops a view, not the
        // name of the material it draws with.
        let mut bytes = Vec::with_capacity(128);
        let counts = [
            mesh.indices.len() as u32,
            u32::from(world.layer(id)),
            u32::from(occluder),
        ];
        counts.iter().for_each(|c| bytes.extend(c.to_le_bytes()));
        bytes.extend(self.geometry(mesh).to_le_bytes());
        let matrix = world.local_to_world(id).to_cols_array();
        matrix
            .iter()
            .for_each(|f| bytes.extend(f.to_bits().to_le_bytes()));
        for ((slot, range), material) in runs.iter().zip(&materials) {
            [*slot as u32, range.start, range.end]
                .iter()
                .for_each(|n| bytes.extend(n.to_le_bytes()));
            bytes.extend(as_occluder(*material));
        }
        Some(Read {
            fingerprint: rusty_core::hash::content(&bytes),
            occluder,
            runs,
            materials,
        })
    }

    /// Entity `id` ready to bake, `None` when it takes no part.
    pub(super) fn object(&self, scene: &Scene, id: u32) -> Option<Object> {
        let mesh = scene.world.mesh(id)?;
        let read = self.read(scene, id, &mesh)?;
        let world = scene.world.local_to_world(id);
        let corner =
            |i: u32| world.transform_point3(Vec3::from(mesh.vertices[i as usize].position));
        let (mut drawn, mut occluding) = (Vec::new(), Vec::new());
        for ((_, range), material) in read.runs.iter().zip(&read.materials) {
            let face = Face::of(world, material.map_or(RenderFace::Front, |m| m.render_face));
            for tri in mesh.indices[range.start as usize..range.end as usize].chunks_exact(3) {
                let tri = [corner(tri[0]), corner(tri[1]), corner(tri[2])];
                drawn.push((tri, face));
                if read.occluder && occludes(*material) {
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
            fingerprint: read.fingerprint,
            drawn,
            occluding,
        })
    }

    /// `mesh`'s [`corners_hash`], computed once per revision.
    fn geometry(&self, mesh: &MeshComponent) -> u64 {
        let mut cache = self.geometry.borrow_mut();
        let entry = cache
            .entry(mesh.revision)
            .or_insert_with(|| (corners_hash(mesh), self.sweep));
        entry.1 = self.sweep;
        entry.0
    }
}

/// `mesh`'s triangles as one hash: each index's corner position, in index order.
/// Corners, not the vertex buffer: unused vertices, and how the buffer welds or
/// splits the corners (normals, UV seams, a re-export), are not hashed, since the
/// bake never reads them (#1958). In index order, so the same corners making other
/// triangles differ. An index past the vertices hashes as NaN corners, so it still
/// differs from any real one.
pub(super) fn corners_hash(mesh: &MeshComponent) -> u64 {
    let mut hash = rusty_core::hash::Content::default();
    for &i in &mesh.indices {
        let corner = mesh
            .vertices
            .get(i as usize)
            .map_or([f32::NAN; 3], |v| v.position);
        corner
            .iter()
            .for_each(|f| hash.write(&f.to_bits().to_le_bytes()));
    }
    hash.finish()
}

/// Whether entity `id`, drawing `mesh`, takes part in the bake: active, static, not
/// skinned (its posed bounds move), not shadows-only (no camera draws it).
pub(super) fn takes_part(scene: &Scene, id: u32, mesh: &MeshComponent) -> bool {
    let world = &scene.world;
    world.is_active_in_hierarchy(id)
        && world.is_static(id)
        && !mesh.is_skinned()
        && mesh.active_palette().is_empty()
        && mesh.cast_shadows.is_visible()
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
pub(super) fn drawn_runs(
    scene: &Scene,
    id: u32,
    mesh: &MeshComponent,
) -> Vec<(usize, std::ops::Range<u32>)> {
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

/// A slot's material as the bake reads it: whether it occludes, and its Render Face.
/// As the fingerprint's bytes: `[occludes, face]`.
pub(super) fn as_occluder(material: Option<&MaterialAsset>) -> [u8; 2] {
    let face = material.map_or(RenderFace::Front, |m| m.render_face);
    [u8::from(occludes(material)), face as u8]
}

/// The material entity `id` draws slot `slot` with, `None` for the engine's default
/// (no material component, or a key the library lacks).
fn slot_material(scene: &Scene, id: u32, slot: usize) -> Option<&MaterialAsset> {
    scene.material_entry_at(id, slot).map(|(_, m)| m)
}

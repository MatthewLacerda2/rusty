//! stored-format: the fingerprint each baked object carries in the `<scene>.occlusion` file
//! The print is taken here in the base, beside the file, because the renderer checks it
//! every frame.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use glam::{Mat4, Vec3};

use crate::Scene;
use rusty_components::mesh::{clip_runs, material_runs, MeshRevision};
use rusty_components::{MaterialAsset, MeshComponent, RenderFace, RenderMode};

/// The side of a triangle that stops a view, by its world winding: the renderer's
/// `Facing` picks the same, so change the two together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
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

    pub fn front(self, normal: Vec3) -> Option<Vec3> {
        match self {
            Self::Ccw => Some(normal),
            Self::Cw => Some(-normal),
            Self::Both => None,
        }
    }

    pub fn stops(self, normal: Vec3, dir: Vec3) -> bool {
        match self {
            Self::Ccw => normal.dot(dir) < 0.0,
            Self::Cw => normal.dot(dir) > 0.0,
            Self::Both => true,
        }
    }
}

pub struct Object {
    pub entity: u32,
    pub fingerprint: u64,
    pub drawn: Vec<([Vec3; 3], Face)>,
    pub occluding: Vec<([Vec3; 3], Face)>,
}

#[derive(Default)]
pub struct Fingerprints {
    grouped: BTreeSet<u32>,
    /// Revision → (corners hash, last sweep). Keyed by revision but hashed by corners, so a
    /// re-weld or re-import under a new revision keeps the print.
    geometry: RefCell<BTreeMap<MeshRevision, (u64, u64)>>,
    sweep: u64,
}

type Run = (usize, std::ops::Range<u32>);

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

    pub fn regroup(&mut self, scene: &Scene) -> bool {
        let now = grouped(scene);
        let changed = now != self.grouped;
        self.grouped = now;
        changed
    }

    pub fn sweep(&mut self) {
        let last = self.sweep;
        self.geometry.get_mut().retain(|_, (_, used)| *used == last);
        self.sweep += 1;
    }

    pub fn of(&self, scene: &Scene, id: u32) -> Option<(u64, bool)> {
        let mesh = scene.world.mesh(id)?;
        let read = self.read(scene, id, &mesh)?;
        Some((read.fingerprint, read.occluder))
    }

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
        // A `LODGroup`'s renderers never occlude: the drawn level moves with the camera.
        let may_occlude =
            !mesh.culls_when_small() && !self.grouped.contains(&id) && !world.has_light(id);
        let occluder = may_occlude && materials.iter().any(|m| occludes(*m));
        // Only what the bake reads: the triangles, not the buffer's layout; each run's side and
        // whether it occludes, never the material's name.
        let mut bytes = Vec::with_capacity(128);
        let counts = [
            mesh.indices.len() as u32,
            u32::from(world.layer(id)),
            u32::from(occluder),
        ];
        counts.iter().for_each(|c| bytes.extend(c.to_le_bytes()));
        bytes.extend(self.geometry(mesh).to_le_bytes());
        let matrix = authored(scene, world.local_to_world(id)).to_cols_array();
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

    pub fn object(&self, scene: &Scene, id: u32) -> Option<Object> {
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

    fn geometry(&self, mesh: &MeshComponent) -> u64 {
        let mut cache = self.geometry.borrow_mut();
        let entry = cache
            .entry(mesh.revision)
            .or_insert_with(|| (corners_hash(mesh), self.sweep));
        entry.1 = self.sweep;
        entry.0
    }
}

fn corners_hash(mesh: &MeshComponent) -> u64 {
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

pub fn takes_part(scene: &Scene, id: u32, mesh: &MeshComponent) -> bool {
    let world = &scene.world;
    world.is_active_in_hierarchy(id)
        && world.is_static(id)
        && !mesh.is_skinned()
        && mesh.active_palette().is_empty()
        && mesh.cast_shadows.is_visible()
}

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

pub fn drawn_runs(
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

fn authored(scene: &Scene, m: glam::Mat4) -> glam::Mat4 {
    if scene.loaded.origin.is_zero() {
        return m;
    }
    let mut m = m;
    let at = m.w_axis.truncate().as_dvec3() + scene.loaded.origin.offset();
    m.w_axis = at.as_vec3().extend(m.w_axis.w);
    m
}

pub fn fingerprint(scene: &Scene, id: u32) -> Option<u64> {
    Fingerprints::of_scene(scene).of(scene, id).map(|(f, _)| f)
}

fn occludes(material: Option<&MaterialAsset>) -> bool {
    material.is_none_or(|m| m.render_mode == RenderMode::Opaque && m.shader.is_none())
}

pub fn as_occluder(material: Option<&MaterialAsset>) -> [u8; 2] {
    let face = material.map_or(RenderFace::Front, |m| m.render_face);
    [u8::from(occludes(material)), face as u8]
}

fn slot_material(scene: &Scene, id: u32, slot: usize) -> Option<&MaterialAsset> {
    scene.material_entry_at(id, slot).map(|(_, m)| m)
}

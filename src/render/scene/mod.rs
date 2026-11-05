//! src/render/scene/ — the retained render scene (#1706): what the renderer keeps of
//! a scene's meshes between frames, so a frame re-derives only what changed.
//!
//! Unreal's `FPrimitiveSceneProxy`, Unity's resident per-material data and Bevy's
//! retained render world do this; before it, every camera rebuilt its draw list from
//! the ECS each frame, hashing and cloning strings per entity.
//!
//! **Objects.** One [`RenderObject`] per mesh entity: its interned mesh
//! ([`MeshHandle`]), resident bounds, world matrix and bounds, flags (shown, static,
//! layer, small-object limits), its object-table slot (world matrix, probe SH,
//! lightmap) and one
//! [`SlotDraw`] per material slot holding the slot's whole batch key (shader, facing,
//! index run, material group, record slot). They are held per scene ([`SceneId`]) in
//! `ids_with_mesh` order, so draws come out in the order the per-entity walk gave.
//!
//! **Extract** (`extract.rs`), once per frame in `prepare_frame`: the change log
//! (`ecs::changes`) names which entities to re-derive. A pose-only change re-places the
//! object ([`Renderer::pose_object`]); anything else re-derives it
//! ([`Renderer::extract_object`]). The log covers only the ECS, so every other input
//! of a draw is either kept with a way to learn it changed, or read per camera on
//! purpose (`inputs.rs` lists which). A frame that changed nothing walks no object.
//!
//! **Per camera** (`draw.rs`): the objects are culled and turned into draw items;
//! only what a camera decides (LOD, culling mask, frustum, small-object cull, view
//! depth, outline, motion history, the reflection probe pick, the proxy volume, a
//! skinned palette) is computed there.
//!
//! **A new per-object input** joins in one of two ways: kept on the object (derive it
//! in `extract_object` or `pose_object`, and make sure whatever changes it makes the
//! extract re-derive the object: a change channel, a scene value's revision in
//! `inputs.rs` (`scene::revision`), or
//! [`Retained::touch`] at the renderer-side write), or read per camera in `draw.rs`.
//! The equivalence tests (`equivalence_tests.rs`) hold that an incrementally updated
//! scene equals one built from scratch; a new input gets a case there.
//!
//! **Shadows** (part 2): the cascades' and the atlas's sweeps gather their casters
//! from the objects (`passes::shadows::casters`), each slot carrying its shadow
//! clip, and the static bakes' key is the scene's [`RenderScene::statics_key`],
//! folded from the objects.
//!
//! **On the GPU** (part 3): each object's `ObjectData` (world matrix, probe SH,
//! lightmap) sits at its slot of the renderer's [`ObjectTable`] (`table.rs`),
//! mirrored on the GPU and written only where it changed; a camera's instance names
//! the slot. A material's param buffer is its scene's own (`MaterialCache`).
//!
//! **Material records** (part 4): each (scene, library material, variant) has its
//! `MaterialData` at a slot of the renderer's [`MaterialRecords`] table (`records.rs`),
//! mirrored on the GPU, and a batch key names the slot. A library edit that moves only
//! a record's values rewrites it in place, no object re-derived; any other edit
//! re-derives only the objects whose slots name the material
//! (`RenderScene::by_material`).

mod draw;
#[cfg(test)]
mod equivalence_ops;
#[cfg(test)]
mod equivalence_tests;
mod extract;
#[cfg(test)]
mod fixture;
#[cfg(test)]
mod gpu_tests;
mod handles;
#[cfg(test)]
mod idle_tests;
mod inputs;
mod object;
pub(crate) mod records;
#[cfg(test)]
mod records_gpu_tests;
#[cfg(test)]
mod shadow_ops;
#[cfg(test)]
mod stack_gpu_tests;
mod table;

use std::collections::{BTreeMap, BTreeSet};

pub(crate) use draw::CameraSolids;
pub(crate) use handles::{MeshHandle, MeshHandles};
pub(crate) use object::{RenderObject, SlotDraw};
pub(crate) use records::{MaterialRecords, Variant};
pub(crate) use table::{ObjectTable, SlotTable, Slotted};

use crate::render::passes::shadows::{casting_key, StaticsKey, UnbakedCasters};
use crate::render::{GpuMesh, Renderer};
use crate::scene::{Scene, SceneId};

/// How many scenes' objects a renderer keeps: the main scene, a preview tab, a
/// capture's. Past it the least recently drawn is dropped, and rebuilt if drawn again.
const MAX_SCENES: usize = 4;

/// The renderer's retained state: the mesh handles, the scenes' objects, and the
/// revision of the renderer-side inputs every key reads.
#[derive(Default)]
pub(crate) struct Retained {
    pub meshes: MeshHandles,
    /// Every kept scene's objects' records, by slot (`table.rs`), mirrored on the GPU.
    pub objects: ObjectTable,
    /// Every kept scene's material records, by slot (`records.rs`), mirrored on the
    /// GPU.
    pub records: MaterialRecords,
    /// Bumped by [`Self::touch`] whenever a renderer-side input every object's keys
    /// read changes (a material group dropped, a surface shader re-built): each scene
    /// re-derives all its objects on its next frame. Render textures have their own
    /// revision, [`Self::render_textures`], so a view switch re-keys only their readers.
    inputs: u64,
    /// Bumped whenever the render textures registered change (`sync_render_textures`):
    /// each scene re-derives the objects whose materials name one. The editor's Scene
    /// and Game views register different sets, so this moves on every switch.
    pub render_textures: u64,
    /// The scenes, least recently drawn first.
    scenes: Vec<(SceneId, RenderScene)>,
}

impl Retained {
    /// A renderer-side input of every key changed.
    pub(crate) fn touch(&mut self) {
        self.inputs += 1;
    }

    /// Take scene `id`'s objects out (empty for a scene not kept), to put back with
    /// [`Self::put`].
    fn take(&mut self, id: SceneId) -> RenderScene {
        match self.scenes.iter().position(|(s, _)| *s == id) {
            Some(at) => self.scenes.remove(at).1,
            None => RenderScene::default(),
        }
    }

    /// Keep `scene` as `id`'s, the most recently drawn; a scene evicted past
    /// [`MAX_SCENES`] gives its object and material slots back, and its id is
    /// returned.
    fn put(&mut self, id: SceneId, scene: RenderScene) -> Option<SceneId> {
        self.scenes.push((id, scene));
        if self.scenes.len() <= MAX_SCENES {
            return None;
        }
        let (gone, mut evicted) = self.scenes.remove(0);
        evicted.release(&mut self.objects);
        self.records.release_scene(gone);
        Some(gone)
    }

    /// Objects kept across every scene: with the identity, the object table's live
    /// slots.
    #[cfg(test)]
    pub(crate) fn kept_objects(&self) -> usize {
        self.scenes.iter().map(|(_, s)| s.objects.len()).sum()
    }

    /// Scene `id`'s objects, if kept.
    pub(crate) fn scene(&self, id: SceneId) -> Option<&RenderScene> {
        self.scenes.iter().find(|(s, _)| *s == id).map(|(_, r)| r)
    }
}

/// One scene's retained objects.
#[derive(Default)]
pub(crate) struct RenderScene {
    /// Its change cursors and snapshots; `None` until its first extract.
    inputs: Option<inputs::Inputs>,
    /// Every mesh entity's object, by id.
    objects: BTreeMap<u32, RenderObject>,
    /// Ids by insertion sequence: `ids_with_mesh`'s order.
    order: BTreeMap<u64, u32>,
    /// Ids by the library names their objects reference
    /// (`RenderObject::materials`): who a library edit re-derives.
    by_material: BTreeMap<String, BTreeSet<u32>>,
    /// The renderers its `LODGroup`s keep out of the static bakes (#1714), re-read
    /// only when a group or a mesh changed.
    pub(crate) unbaked: UnbakedCasters,
    /// Its static casters' digest (`passes::shadows::casting_key`), refolded from
    /// the objects only after a static one was re-derived or `unbaked` changed;
    /// `None` until then.
    casting: Option<u64>,
}

impl RenderScene {
    /// The objects in `ids_with_mesh` order, with their ids.
    pub(crate) fn objects(&self) -> impl Iterator<Item = (u32, &RenderObject)> {
        self.order.values().map(|&id| (id, &self.objects[&id]))
    }

    /// Replace (or, with `None`, remove) entity `id`'s object.
    fn set(&mut self, id: u32, object: Option<RenderObject>) {
        if let Some(old) = self.objects.remove(&id) {
            self.order.remove(&old.seq);
            self.forget_casting(old.is_static);
            for name in &old.materials {
                let users = self.by_material.get_mut(name).expect("indexed with it");
                users.remove(&id);
                if users.is_empty() {
                    self.by_material.remove(name);
                }
            }
        }
        if let Some(object) = object {
            self.forget_casting(object.is_static);
            self.order.insert(object.seq, id);
            for name in &object.materials {
                self.by_material.entry(name.clone()).or_default().insert(id);
            }
            self.objects.insert(id, object);
        }
    }

    /// The objects drawing any library material in `names`.
    fn users_of<'a>(&'a self, names: &'a BTreeSet<String>) -> impl Iterator<Item = u32> + 'a {
        names
            .iter()
            .filter_map(|name| self.by_material.get(name))
            .flatten()
            .copied()
    }

    /// The objects drawing library material `name`.
    fn users_of_name<'a>(&'a self, name: &str) -> impl Iterator<Item = u32> + 'a {
        self.by_material.get(name).into_iter().flatten().copied()
    }

    /// Drop every object, giving its slot back to `table`.
    pub(super) fn release(&mut self, table: &mut ObjectTable) {
        for object in self.objects.values() {
            table.release(object.slot);
        }
        self.objects.clear();
        self.order.clear();
        self.by_material.clear();
        self.casting = None;
    }

    /// A static object came or went: the digest is refolded.
    fn forget_casting(&mut self, is_static: bool) {
        if is_static {
            self.casting = None;
        }
    }

    /// `unbaked` re-read: the digest is refolded when it moved.
    fn regroup(&mut self, unbaked: UnbakedCasters) {
        if unbaked != self.unbaked {
            self.unbaked = unbaked;
            self.casting = None;
        }
    }

    /// The digest, refolded if forgotten.
    fn refold_casting(&mut self) {
        if self.casting.is_none() {
            let statics = self.objects().filter(|(_, o)| o.is_static);
            let statics = statics.map(|(id, o)| (id, &o.casting));
            self.casting = Some(casting_key(statics, &self.unbaked));
        }
    }

    /// The static bakes' key for `scene` (whose objects these are) now: what a
    /// cascade layer or an atlas tile keeps beside what it baked.
    pub(crate) fn statics_key(&self, scene: &Scene) -> StaticsKey {
        let casting = self.casting.expect("an extract folds the digest");
        (scene.id(), scene.world.static_revision(), casting)
    }
}

/// What one extract did. `extract_scene` adds it to the frame's
/// `RenderCounters`, which `Debug.Stats` reports as `render_extracted` and
/// `render_posed` (`render::stats`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExtractStats {
    /// Objects re-derived whole: ids the extract re-read that still have a mesh.
    pub extracted: u32,
    /// Objects only re-placed (a move).
    pub posed: u32,
}

impl Renderer {
    /// Keep `rs` as scene `id`'s objects. A scene that evicts gives back its slots
    /// and its material param buffers (`MaterialCache::release_scene`).
    fn keep_scene(&mut self, id: SceneId, rs: RenderScene) {
        if let Some(gone) = self.retained.put(id, rs) {
            self.materials.release_scene(gone);
        }
    }

    /// The resident buffers of the mesh `handle` names.
    pub(crate) fn gpu_mesh_of(&self, handle: MeshHandle) -> Option<&GpuMesh> {
        self.gpu_meshes.get(self.retained.meshes.id(handle))
    }
}

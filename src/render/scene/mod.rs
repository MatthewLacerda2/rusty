//! src/render/scene/ — the retained render scene (#1706): what the renderer keeps of
//! a scene's meshes between frames, so a frame re-derives only what changed.
//!
//! Unreal's `FPrimitiveSceneProxy`, Unity's resident per-material data and Bevy's
//! retained render world do this; before it, every camera rebuilt its draw list from
//! the ECS each frame, hashing and cloning strings per entity.
//!
//! **Objects.** One [`RenderObject`] per mesh entity: its interned mesh
//! ([`MeshHandle`]), resident bounds, world matrix and bounds, flags (shown, static,
//! layer, small-object limits), its instance (world matrix, probe SH, lightmap) and one
//! [`SlotDraw`] per material slot holding the slot's whole batch key (shader, facing,
//! index run, material group, record). They are held per scene ([`SceneId`]) in
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
//! Part 2 of #1706 moves the shadow walks, the decals and a persistent material
//! table onto these objects; they still walk the ECS themselves.

mod draw;
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

use std::collections::BTreeMap;

pub(crate) use draw::CameraSolids;
pub(crate) use handles::{MeshHandle, MeshHandles};
pub(crate) use object::RenderObject;

use crate::render::{GpuMesh, Renderer};
use crate::scene::SceneId;

/// How many scenes' objects a renderer keeps: the main scene, a preview tab, a
/// capture's. Past it the least recently drawn is dropped, and rebuilt if drawn again.
const MAX_SCENES: usize = 4;

/// The renderer's retained state: the mesh handles, the scenes' objects, and the
/// revision of the renderer-side inputs every key reads.
#[derive(Default)]
pub(crate) struct Retained {
    pub meshes: MeshHandles,
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

    /// Keep `scene` as `id`'s, the most recently drawn.
    fn put(&mut self, id: SceneId, scene: RenderScene) {
        self.scenes.push((id, scene));
        if self.scenes.len() > MAX_SCENES {
            self.scenes.remove(0);
        }
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
        }
        if let Some(object) = object {
            self.order.insert(object.seq, id);
            self.objects.insert(id, object);
        }
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
    /// The resident buffers of the mesh `handle` names.
    pub(crate) fn gpu_mesh_of(&self, handle: MeshHandle) -> Option<&GpuMesh> {
        self.gpu_meshes.get(self.retained.meshes.id(handle))
    }
}

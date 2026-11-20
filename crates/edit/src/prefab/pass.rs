//! crates/edit/src/prefab/pass.rs — one propagation pass over linked instances: the
//! source baselines it builds once per source, and the model lookups it shares.
//!
//! Rebuilding an instance needs its source's fresh baselines (the source stamped
//! into a scratch scene, its meshes rehydrated). They are the same for every
//! instance of one source, so a pass builds them once per source and hands each
//! instance a copy without geometry: the instance's part keeps its live geometry
//! where that is still what the source builds, and is rebuilt from its reference
//! otherwise (`link::rebuild_entity`). Every model file the pass reads (a model
//! source, a mesh of a baseline, a part rebuilt) is looked up once, through the
//! [`Imports`] the pass was handed: a load's or a Stop's, so the rehydrate before
//! the propagation and the propagation share one lookup per file.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::authoring::material::drop_unused;
use crate::prefab::source::read_source_in;
use crate::prefab::{stamp_prefab, Merge};
use crate::PrefabData;
use rusty_components::{Entity, EntityRefs};
use rusty_scene::rehydrate::Imports;
use rusty_scene::Scene;

/// One pass's sources, baselines and lookups.
pub(super) struct Pass<'a> {
    pub(super) imports: &'a mut Imports,
    /// Whether a document is being applied (a load, a Play snapshot:
    /// `structure::reshape`).
    pub(super) applying: bool,
    sources: BTreeMap<String, Option<Rc<PrefabData>>>,
    baselines: BTreeMap<String, BTreeMap<u32, Entity>>,
    /// The material keys the baselines' merges added, dropped at [`Self::finish`]
    /// where no slot names them (#1576).
    added: Vec<String>,
}

impl<'a> Pass<'a> {
    pub(super) fn new(imports: &'a mut Imports, applying: bool) -> Self {
        Self {
            imports,
            applying,
            sources: BTreeMap::new(),
            baselines: BTreeMap::new(),
            added: Vec::new(),
        }
    }

    /// The source at `path`, read once per pass and shared by every instance of
    /// it (never copied per instance); `None` when it can't be read.
    pub(super) fn source(&mut self, path: &str) -> Option<Rc<PrefabData>> {
        let imports = &mut *self.imports;
        let read = |p: &String| read_source_in(p, imports).ok().map(Rc::new);
        self.sources
            .entry(path.to_string())
            .or_insert_with_key(read)
            .clone()
    }

    /// The fresh baselines of `source` (read as `prefab`), keyed by source
    /// `local_id`, without mesh geometry: built once per pass ([`fresh_baselines`]).
    pub(super) fn baselines(
        &mut self,
        scene: &mut Scene,
        source: &str,
        prefab: &PrefabData,
    ) -> BTreeMap<u32, Entity> {
        if let Some(built) = self.baselines.get(source) {
            return built.clone();
        }
        let (built, added) = fresh_baselines(scene, source, prefab, self.imports);
        self.added.extend(added);
        self.baselines.insert(source.to_string(), built.clone());
        built
    }

    /// End the pass: only what a slot still names after the overrides stays, so a
    /// material deleted from the scene is not brought back by its prefab's copy
    /// (#1576).
    pub(super) fn finish(self, scene: &mut Scene) {
        drop_unused(scene, &self.added);
    }
}

/// Build the fresh source baselines of `source`, keyed by source `local_id`.
/// Instantiates the source into a SCRATCH scene that borrows `scene`'s material
/// library (moved in and back, never cloned), so the merge reuses its entries and
/// rewrites references to the very same library keys the instance uses, and what
/// it adds comes back with it. A model's keys resolve to the scene's entry as it
/// stands, an edited one included (`Merge::KeepScene`, #1637). The returned
/// entities are recorded as the Undo journal records them, without mesh geometry
/// (`World::record`), and carry their LOCAL ids, component references included
/// (callers move those into the instance's ids with `link::in_instance_space` and
/// overwrite identity/structure). A reference to one of the scratch skeleton's
/// bones keeps the bone by name in `bone_refs` (#1264), naming its skinned entity
/// meanwhile. Also returns the keys the merge added.
fn fresh_baselines(
    scene: &mut Scene,
    source: &str,
    prefab: &PrefabData,
    imports: &mut Imports,
) -> (BTreeMap<u32, Entity>, Vec<String>) {
    let mut scratch = Scene::new();
    scratch.materials = std::mem::take(&mut scene.materials);
    let merge = Merge::for_source(source);
    let (root, added) = stamp_prefab(&mut scratch, prefab, None, None, merge, imports);
    scene.materials = std::mem::take(&mut scratch.materials);

    // Map each scratch entity back to its source local id via subtree order: the
    // instantiate assigned local `l` -> `base + l`, so subtracting the base recovers
    // `l`. The base is the scratch root's id minus the prefab root's local id.
    let base = root.wrapping_sub(prefab.root);
    let mut docs: Vec<Entity> = scratch
        .entity_ids()
        .into_iter()
        .filter_map(|id| scratch.world.record(id).map(|r| r.doc))
        .collect();
    scratch.bone_doc_refs(&mut docs);
    let mut out = BTreeMap::new();
    for mut entity in docs {
        entity.remap_refs(&|r| Some(r.wrapping_sub(base)));
        out.insert(entity.id.wrapping_sub(base), entity);
    }
    (out, added)
}

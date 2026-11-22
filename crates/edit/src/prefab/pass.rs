//! The baselines are the same for every instance of one source, so a pass builds them once and
//! hands each instance a copy without geometry.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::authoring::material::drop_unused;
use crate::prefab::source::read_source_in;
use crate::prefab::{stamp_prefab, Merge};
use crate::PrefabData;
use rusty_components::{Entity, EntityRefs};
use rusty_scene::rehydrate::Imports;
use rusty_scene::Scene;

pub(super) struct Pass<'a> {
    pub(super) imports: &'a mut Imports,
    pub(super) applying: bool,
    sources: BTreeMap<String, Option<Rc<PrefabData>>>,
    baselines: BTreeMap<String, BTreeMap<u32, Entity>>,
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

    pub(super) fn source(&mut self, path: &str) -> Option<Rc<PrefabData>> {
        let imports = &mut *self.imports;
        let read = |p: &String| read_source_in(p, imports).ok().map(Rc::new);
        self.sources
            .entry(path.to_string())
            .or_insert_with_key(read)
            .clone()
    }

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

    /// A material deleted from the scene must not be brought back by its prefab's copy.
    pub(super) fn finish(self, scene: &mut Scene) {
        drop_unused(scene, &self.added);
    }
}

/// Instantiates into a SCRATCH scene that borrows `scene`'s material library (moved in and back,
/// never cloned), so references resolve to the very library keys the instance uses. A reference to
/// one of the scratch skeleton's bones keeps the bone by name in `bone_refs`.
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

    // The instantiate assigned local `l` -> `base + l`.
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

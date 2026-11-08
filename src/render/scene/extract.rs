//! src/render/scene/extract.rs — the one extract step a frame takes (#1706): bring
//! the drawn scene's retained objects up to date from what changed, uploading the
//! meshes and loading the maps of the objects it re-derives first, so their keys bind
//! what is resident.

use std::collections::BTreeSet;

use super::inputs::{Dirty, Inputs, LibraryEdits, RendererInputs};
use super::RenderScene;
use crate::render::draw::materials::material_maps;
use crate::render::gpu::material_cache::ParamOwner;
use crate::render::passes::shadows::UnbakedCasters;
use crate::render::Renderer;
use crate::scene::{Scene, SceneId};

impl Renderer {
    /// Bring `scene`'s retained objects up to date. Once per frame, after everything
    /// a key or instance reads on the renderer is settled (`prepare_frame`).
    pub(crate) fn extract_scene(&mut self, scene: &Scene) {
        let start = std::time::Instant::now();
        let mut retained = self.retained.take(scene.id());
        let last = self.update_objects(scene, &mut retained);
        self.keep_scene(scene.id(), retained);
        self.frame_counters.render_extracted += last.extracted;
        self.frame_counters.render_posed += last.posed;
        self.frame_counters.render_extract_us += start.elapsed().as_micros() as u64;
    }

    /// The renderer-side inputs every object reads, as they are now.
    fn renderer_inputs(&self) -> RendererInputs {
        RendererInputs {
            revision: self.retained.inputs,
            lightmaps_resident: self.lightmaps.resident(),
            lightmaps_directional: self.lightmaps.directional(),
            static_only: self.capture.static_only,
            probe_bounce: self.capture.probe_bounce,
        }
    }

    fn update_objects(&mut self, scene: &Scene, rs: &mut RenderScene) -> ExtractStats {
        let now = self.renderer_inputs();
        let (dirty, library) = match &mut rs.inputs {
            Some(inputs) => inputs.read(scene, now),
            None => {
                rs.inputs = Some(Inputs::open(scene, now));
                (Dirty::Rebuild, LibraryEdits::default())
            }
        };
        let rekeyed = self.apply_library(scene, rs, &library);
        // Moving between two ticks moves what is drawn with no log entry (#1877); a
        // rebuild poses them all anyway, but the set drawn blended is noted either way.
        let between_ticks = self.draw_poses.repose(scene);
        let stats = match dirty {
            Dirty::Rebuild => self.rebuild_objects(scene, rs),
            Dirty::Some {
                mut whole,
                mut posed,
            } => {
                whole.extend(rekeyed);
                posed.extend(between_ticks);
                self.update_some(scene, rs, whole, posed)
            }
        };
        let inputs = rs.inputs.as_mut().expect("opened above");
        inputs.uploads_seen = self.retained.meshes.uploads();
        inputs.render_textures_seen = self.retained.render_textures;
        if inputs.regroup {
            rs.regroup(scene, UnbakedCasters::of(scene));
        }
        rs.refold_casting();
        rs.occlusion.refresh(scene);
        rs.refresh_casters(scene);
        stats
    }

    /// Act on the library's edits (#1706): a removed name's records are freed, and a
    /// record-only edit rewrites the name's records in the material table and its
    /// param buffer in place. Returns the objects to re-derive: the users of a name
    /// added, removed or re-keyed, and those of a name whose params moved that
    /// override them (their buffers merge the material's params).
    fn apply_library(
        &mut self,
        scene: &Scene,
        rs: &RenderScene,
        edits: &LibraryEdits,
    ) -> BTreeSet<u32> {
        let mut rederive: BTreeSet<u32> = rs.users_of(&edits.rekeyed).collect();
        for name in &edits.rekeyed {
            if !scene.materials.contains_key(name) {
                self.retained.records.release_name(scene.id(), name);
            }
        }
        for (name, &params) in &edits.rewritten {
            let Some(asset) = scene.materials.get(name) else {
                continue;
            };
            for (variant, slot) in self.retained.records.variants_of(scene.id(), name) {
                let record = self.variant_record(variant, Some(asset));
                self.retained.records.write(slot, record);
            }
            if !params {
                continue;
            }
            let overriding = rs.users_of_name(name);
            rederive.extend(overriding.filter(|&id| scene.shader_overrides.of(id).is_some()));
            // Only a buffer something draws is rewritten, so a params edit on a
            // material nothing draws compiles no shader mid-extract.
            let owner = ParamOwner::Material(scene.id(), name.clone());
            if !self.materials.holds_params(&owner) {
                continue;
            }
            let shader = asset.shader.as_deref();
            let pipeline = self.surface_shaders.pipeline_id(&self.device, shader);
            if let Some(layout) = self.surface_shaders.params(pipeline) {
                let values = layout.pack(&asset.shader_params);
                let (device, queue) = (&self.device, &self.queue);
                self.materials.write_params(device, queue, &owner, values);
            }
        }
        rederive
    }

    /// Every mesh entity re-derived: the first frame, a load, an Undo, a renderer
    /// input. The one walk of the World an extract takes. An object still there
    /// keeps its slot, so a record whose value did not move uploads nothing; only
    /// the slots of objects gone are freed.
    pub(super) fn rebuild_objects(&mut self, scene: &Scene, rs: &mut RenderScene) -> ExtractStats {
        let ids = scene.world.ids_with_mesh();
        self.upload_meshes_of(scene, &ids);
        self.load_maps_of(scene, &ids);
        let meshed: BTreeSet<u32> = ids.iter().copied().collect();
        let gone: Vec<u32> = rs
            .objects
            .keys()
            .filter(|id| !meshed.contains(id))
            .copied()
            .collect();
        for id in gone {
            let slot = rs.objects[&id].slot;
            self.retained.objects.release(slot);
            rs.set(id, None);
        }
        rs.casting = None;
        rs.occlusion.forget();
        ExtractStats {
            extracted: self.derive_objects(scene, rs, &ids),
            posed: 0,
        }
    }

    fn update_some(
        &mut self,
        scene: &Scene,
        rs: &mut RenderScene,
        mut whole: BTreeSet<u32>,
        posed: BTreeSet<u32>,
    ) -> ExtractStats {
        let changed: Vec<u32> = whole.iter().copied().collect();
        self.upload_meshes_of(scene, &changed);
        // A geometry re-uploaded since the last extract (by this scene, another or a
        // mesh particle) re-shapes every object drawing it. Walked only then.
        let seen = rs.inputs.as_ref().map_or(0, |i| i.uploads_seen);
        if self.retained.meshes.uploads() > seen {
            let meshes = &self.retained.meshes;
            let reshaped = rs.objects.iter();
            let reshaped = reshaped.filter(|(_, o)| meshes.uploaded_since(o.mesh, seen));
            whole.extend(reshaped.map(|(&id, _)| id));
        }
        // The views re-registered their render textures: re-key what binds one.
        if rs
            .inputs
            .as_ref()
            .is_some_and(|i| i.render_textures_seen != self.retained.render_textures)
        {
            let reads = rs.objects.iter().filter(|(_, o)| o.reads_render_texture);
            whole.extend(reads.map(|(&id, _)| id));
        }
        let whole: Vec<u32> = whole.into_iter().collect();
        self.load_maps_of(scene, &whole);
        let extracted = self.derive_objects(scene, rs, &whole);
        let mut moved = 0;
        for id in posed
            .into_iter()
            .filter(|id| whole.binary_search(id).is_err())
        {
            if let Some(object) = rs.objects.get_mut(&id) {
                self.pose_object(scene, id, object);
                moved += 1;
                if object.is_static {
                    rs.occlusion.touch(id);
                }
            }
        }
        ExtractStats {
            extracted,
            posed: moved,
        }
    }

    /// Re-derive the objects of `ids`, each keeping its slot (an id without a mesh
    /// drops its object and frees its slot); how many were derived.
    fn derive_objects(&mut self, scene: &Scene, rs: &mut RenderScene, ids: &[u32]) -> u32 {
        let mut derived = 0;
        for &id in ids {
            let slot = rs.objects.get(&id).map(|o| o.slot);
            let object = self.extract_object(scene, id, slot);
            match (&object, slot) {
                (None, Some(slot)) => self.retained.objects.release(slot),
                _ => derived += u32::from(object.is_some()),
            }
            rs.set(id, object);
        }
        derived
    }

    /// Load every map the materials of `entities` sample (albedo #201, metallic and
    /// roughness #202, the rest and the shader texture slots #400), each as what its
    /// slot holds (#1322), before their keys are built. Inactive entities' too:
    /// re-enabling one must not stall on uploads and mip generation (#1363).
    fn load_maps_of(&mut self, scene: &Scene, entities: &[u32]) {
        for &id in entities {
            let maps = scene
                .material_assets_of(id)
                .flat_map(|m| material_maps(Some(m)));
            for (path, kind) in maps {
                self.load_texture_as(&path, kind);
            }
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
    /// Keep `rs` as scene `id`'s objects. A scene that evicts gives back its slots
    /// and its material param buffers (`MaterialCache::release_scene`).
    pub(super) fn keep_scene(&mut self, id: SceneId, rs: RenderScene) {
        if let Some(gone) = self.retained.put(id, rs) {
            self.materials.release_scene(gone);
        }
    }
}

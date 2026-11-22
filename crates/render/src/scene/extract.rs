use std::collections::BTreeSet;

use super::inputs::{Dirty, Inputs, LibraryEdits, RendererInputs};
use super::RenderScene;
use crate::draw::map_slots::material_maps;
use crate::gpu::material_cache::ParamOwner;
use crate::passes::shadows::UnbakedCasters;
use crate::Renderer;
use rusty_scene::{Scene, SceneId};

impl Renderer {
    pub(crate) fn extract_scene(&mut self, scene: &Scene) {
        let start = std::time::Instant::now();
        let mut retained = self.retained.take(scene.id());
        let last = self.update_objects(scene, &mut retained);
        self.free_deleted_meshes(scene, &mut retained);
        self.keep_scene(scene.id(), retained);
        self.frame_counters.render_extracted += last.extracted;
        self.frame_counters.render_posed += last.posed;
        self.frame_counters.render_extract_us += start.elapsed().as_micros() as u64;
    }

    fn free_deleted_meshes(&mut self, scene: &Scene, rs: &mut RenderScene) {
        if scene.meshes.keys().eq(rs.library_meshes.iter()) {
            return;
        }
        for name in &rs.library_meshes {
            if !scene.meshes.contains_key(name) {
                let kind = rusty_components::mesh::SCENE_KIND;
                let id = crate::MeshId(format!("{kind}|{name}"));
                self.gpu_meshes.remove(&id);
                self.retained.residency.refused.remove(&id);
            }
        }
        rs.library_meshes = scene.meshes.keys().cloned().collect();
    }

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
        let seen = rs.inputs.as_ref().map_or(0, |i| i.uploads_seen);
        if self.retained.meshes.uploads() > seen {
            let meshes = &self.retained.meshes;
            let reshaped = rs.objects.iter();
            let reshaped = reshaped.filter(|(_, o)| meshes.uploaded_since(o.mesh, seen));
            whole.extend(reshaped.map(|(&id, _)| id));
        }
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
                    rs.casters.touch(id);
                    rs.rays.touch();
                }
            }
        }
        ExtractStats {
            extracted,
            posed: moved,
        }
    }

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExtractStats {
    pub extracted: u32,
    pub posed: u32,
}

impl Renderer {
    pub(super) fn keep_scene(&mut self, id: SceneId, rs: RenderScene) {
        if let Some(gone) = self.retained.put(id, rs) {
            self.materials.release_scene(gone);
            self.retained.residency.forget(gone);
        }
    }
}

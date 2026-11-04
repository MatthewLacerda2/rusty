//! src/render/scene/extract.rs — the one extract step a frame takes (#1706): bring
//! the drawn scene's retained objects up to date from what changed, uploading the
//! meshes and loading the maps of the objects it re-derives first, so their keys bind
//! what is resident.

use std::collections::BTreeSet;

use super::inputs::{Dirty, Inputs, RendererInputs};
use super::{ExtractStats, RenderScene};
use crate::render::draw::materials::material_maps;
use crate::render::passes::shadows::UnbakedCasters;
use crate::render::Renderer;
use crate::scene::Scene;

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
        let dirty = match &mut rs.inputs {
            Some(inputs) => inputs.read(scene, now),
            None => {
                rs.inputs = Some(Inputs::open(scene, now));
                Dirty::Rebuild
            }
        };
        let stats = match dirty {
            Dirty::Rebuild => self.rebuild_objects(scene, rs),
            Dirty::Some {
                mut whole,
                posed,
                materials,
            } => {
                whole.extend(rs.users_of(&materials));
                self.update_some(scene, rs, whole, posed)
            }
        };
        let inputs = rs.inputs.as_mut().expect("opened above");
        inputs.uploads_seen = self.retained.meshes.uploads();
        inputs.render_textures_seen = self.retained.render_textures;
        if inputs.regroup {
            rs.regroup(UnbakedCasters::of(scene));
        }
        rs.refold_casting();
        stats
    }

    /// Every mesh entity re-derived: the first frame, a load, an Undo, a renderer
    /// input. The one walk of the World an extract takes.
    pub(super) fn rebuild_objects(&mut self, scene: &Scene, rs: &mut RenderScene) -> ExtractStats {
        let ids = scene.world.ids_with_mesh();
        self.upload_meshes_of(scene, &ids);
        self.load_maps_of(scene, &ids);
        rs.release(&mut self.retained.objects);
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

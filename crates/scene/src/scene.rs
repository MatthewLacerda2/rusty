use std::collections::BTreeMap;

use glam::Vec3;

use crate::collision_matrix::CollisionMatrix;
use crate::identity::SceneId;
use crate::layers::LayerRegistry;
use crate::nav_settings::NavMeshSettings;
use rusty_ecs::World;

pub use rusty_components::{
    AnimatorComponent, AudioSourceComponent, CameraComponent, ClearFlags, ColliderComponent,
    ColliderShape, CollisionDetection, CollisionResponse, EmitMode, Entity, LightComponent,
    LightMode, LightType, MaterialAsset, MaterialComponent, MeshComponent, MeshRevision,
    NavMeshAgentComponent, Particle, ParticleBlend, ParticleEmitterComponent,
    PostProcessingComponent, RenderMode, RigidBodyComponent, ScriptComponent, ScriptFieldValue,
    Tonemap, TransformComponent,
};

fn default_skybox_path() -> String {
    "".to_string()
}

/// Hemisphere ambient's sky (up) colour; the default gradient's other two scale it.
pub const DEFAULT_AMBIENT_COLOR: Vec3 = Vec3::new(0.45, 0.55, 0.70);
/// Unitless, on the ambient colour.
pub const DEFAULT_AMBIENT_INTENSITY: f32 = 0.55;

pub struct Scene {
    pub(crate) id: SceneId,
    pub world: World,
    pub selected_entity_id: Option<u32>,
    pub skybox_path: String,
    pub ambient_color: Vec3,
    pub ambient_intensity: f32,
    pub ambient: crate::AmbientSettings,
    pub nav_settings: NavMeshSettings,
    pub fog: crate::FogSettings,
    pub sky: crate::SkySettings,
    pub layers: LayerRegistry,
    pub collision_matrix: CollisionMatrix,
    pub materials: crate::revision::Revisioned<BTreeMap<String, MaterialAsset>>,
    pub meshes: crate::mesh_library::MeshLibrary,
    pub decals: crate::decal::DecalSet,
    pub probes: crate::lighting::probe::ProbeVolume,
    pub reflection_probes: crate::lighting::reflection_probe::ReflectionProbeSet,
    pub lightmaps: crate::revision::Revisioned<crate::lighting::lightmap_set::LightmapSet>,
    pub occlusion: crate::lighting::occlusion::SceneOcclusion,
    pub navmesh: crate::nav_settings::SavedNavmesh,
    pub(crate) bake_files: crate::io::BakeFiles,
    pub lighting_settings: crate::lighting::LightingSettings,
    pub pending_destroy: Vec<u32>,
    pub(crate) playing: bool,
    pub loads: crate::runtime::loads::SceneLoads,
    pub persistent: std::collections::BTreeSet<u32>,
    /// The scene files loaded into the World and which one each root belongs to.
    pub loaded: crate::runtime::membership::LoadedScenes,
    pub debug_lines: crate::runtime::debug_lines::DebugLines,
    pub shader_time: f32,
    /// `Time::unscaled_time`: runs at time scale 0 too.
    pub ui_time: f32,
    pub inspected_reflection_probe: Option<usize>,
    #[cfg(feature = "dev")]
    /// GPU-metered, so it differs per GPU: never a sim input, only display and `Debug`.
    pub screen_exposure: Option<f32>,
    #[cfg(feature = "dev")]
    pub lighting_bake_running: bool,
    #[cfg(feature = "dev")]
    /// `Lighting.GenerateAsync()` waiting for the editor's next frame; Play refuses meanwhile.
    pub lighting_bake_requested: bool,
    #[cfg(feature = "dev")]
    pub lighting_bake_status: crate::lighting::bake_status::BakeStatus,
    #[cfg(feature = "dev")]
    pub lighting_queue_requested: bool,
    #[cfg(feature = "dev")]
    pub lighting_queue: Option<crate::lighting::bake_status::QueueSpot>,
    pub history_cuts: crate::camera::HistoryCuts,
    pub shader_overrides: crate::shader_overrides::ShaderOverrides,
    pub ragdolls: crate::skeleton::RagdollRuntime,
    pub mesh_reads: crate::mesh_library::MeshReads,
    pub(crate) disk_text: crate::io::DiskText,
    pub(crate) history: crate::undo::UndoStack,
}

impl Default for Scene {
    #[expect(
        clippy::too_many_lines,
        reason = "a flat one-line-per-field constructor: its length is the struct's width"
    )]
    fn default() -> Self {
        Self {
            id: SceneId::next(),
            world: World::new(),
            selected_entity_id: None,
            skybox_path: default_skybox_path(),
            ambient_color: DEFAULT_AMBIENT_COLOR,
            ambient_intensity: DEFAULT_AMBIENT_INTENSITY,
            ambient: Default::default(),
            nav_settings: NavMeshSettings::default(),
            fog: crate::FogSettings::default(),
            sky: crate::SkySettings::default(),
            layers: LayerRegistry::default(),
            collision_matrix: CollisionMatrix::default(),
            materials: BTreeMap::new().into(),
            meshes: BTreeMap::new(),
            decals: Default::default(),
            probes: crate::lighting::probe::ProbeVolume::new(),
            reflection_probes: crate::lighting::reflection_probe::ReflectionProbeSet::new(),
            lightmaps: Default::default(),
            occlusion: Default::default(),
            navmesh: Default::default(),
            bake_files: Default::default(),
            lighting_settings: Default::default(),
            pending_destroy: Vec::new(),
            playing: false,
            loads: Default::default(),
            persistent: std::collections::BTreeSet::new(),
            loaded: Default::default(),
            debug_lines: Default::default(),
            shader_time: 0.0,
            ui_time: 0.0,
            inspected_reflection_probe: None,
            #[cfg(feature = "dev")]
            screen_exposure: None,
            #[cfg(feature = "dev")]
            lighting_bake_running: false,
            #[cfg(feature = "dev")]
            lighting_bake_requested: false,
            #[cfg(feature = "dev")]
            lighting_bake_status: Default::default(),
            #[cfg(feature = "dev")]
            lighting_queue_requested: false,
            #[cfg(feature = "dev")]
            lighting_queue: None,
            history_cuts: Default::default(),
            shader_overrides: Default::default(),
            ragdolls: Default::default(),
            mesh_reads: Default::default(),
            disk_text: Default::default(),
            history: Default::default(),
        }
    }
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_entity(&mut self, name: String) -> u32 {
        self.world.spawn(name)
    }

    pub fn find_entity_by_name(&self, name: &str) -> Option<u32> {
        self.world.find_by_name(name)
    }

    pub fn entity_ids(&self) -> Vec<u32> {
        self.world.ids().to_vec()
    }

    pub fn entity_count(&self) -> usize {
        self.world.len()
    }

    pub fn is_empty(&self) -> bool {
        self.world.is_empty()
    }

    pub fn set_parent(&mut self, entity_id: u32, parent_id: Option<u32>) -> Result<(), String> {
        if let Some(p_id) = parent_id {
            if entity_id == p_id {
                return Err("An entity cannot be parented to itself.".to_string());
            }
            if self.world.ancestors(p_id).any(|a| a == entity_id) {
                return Err("Circular parenting detected (ancestor loop).".to_string());
            }
        }

        // Validate before touching a link: a half-applied reparent leaves a one-sided link the
        // store never dirties again.
        if !self.world.contains(entity_id) {
            return Err("Entity not found.".to_string());
        }
        if parent_id.is_some_and(|p| !self.world.contains(p)) {
            return Err("Parent entity not found.".to_string());
        }
        if let Some(old_p) = self.world.parent_id(entity_id) {
            self.world.remove_child(old_p, entity_id);
        }

        if let Some(new_p) = parent_id {
            self.world.add_child(new_p, entity_id);
        }

        // Keeps the local transform: the subtree moves in world space.
        self.world.set_parent_id(entity_id, parent_id);

        Ok(())
    }
}

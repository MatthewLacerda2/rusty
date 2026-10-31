//! src/scene/undo/level.rs — the scene-level "object" an undo step records (#1631).
//!
//! Scene Settings, fog, layers, the collision matrix, the material library, the
//! probe sets and the bake's settings live on `Scene`, beside the World, and are
//! written by plain field assignment from the inspector and the API alike. They
//! are recorded as one more object: the stack keeps their serialized bytes as a
//! baseline, and an edit whose bytes differ from it records the baseline's values
//! as the state before. One list of fields, so the stored copy, the compared bytes
//! and the restore never disagree about what the scene level is.

use serde::Serialize;

use crate::scene::Scene;

macro_rules! scene_level {
    ($($field:ident: $ty:ty,)*) => {
        /// The scene-level state, as values: what a step puts back.
        #[derive(Clone, Debug, Serialize)]
        pub(super) struct SceneLevel {
            $($field: $ty,)*
        }

        /// The same fields borrowed: what is serialized to compare, uncopied.
        #[derive(Serialize)]
        struct LevelRef<'a> {
            $($field: &'a $ty,)*
        }

        impl SceneLevel {
            /// `scene`'s scene-level state, copied.
            pub(super) fn capture(scene: &Scene) -> Self {
                Self { $($field: scene.$field.clone(),)* }
            }

            /// Put these values back on `scene`.
            pub(super) fn restore(self, scene: &mut Scene) {
                $(scene.$field = self.$field;)*
            }
        }

        /// `scene`'s scene-level state serialized into `out` (cleared first).
        pub(super) fn level_bytes(scene: &Scene, out: &mut Vec<u8>) {
            out.clear();
            let level = LevelRef { $($field: &scene.$field,)* };
            // Plain data, written to memory: serializing it cannot fail.
            let _ = serde_json::to_writer(out, &level);
        }
    };
}

scene_level! {
    skybox_path: String,
    ambient_color: glam::Vec3,
    ambient_intensity: f32,
    nav_settings: crate::scene::nav_settings::NavMeshSettings,
    fog: crate::scene::FogSettings,
    layers: crate::scene::LayerRegistry,
    collision_matrix: crate::scene::CollisionMatrix,
    materials: std::collections::BTreeMap<String, crate::scene::MaterialAsset>,
    probes: crate::scene::lighting::probe::ProbeVolume,
    reflection_probes: crate::scene::lighting::reflection_probe::ReflectionProbeSet,
    lightmaps: crate::scene::lighting::lightmap::LightmapSet,
    lighting_settings: crate::scene::lighting::LightingSettings,
}

use super::{Class, Field, Keyed};
use crate::Scene;

/// The authored fields, handed to `$then!`: `same` says whether a value is unchanged as an
/// edit sees it, `put` puts a recorded one back (both in `undo::level`, which expands this).
macro_rules! authored_fields {
    ($then:ident { $($rest:tt)* }) => {
        $then! {
            authored {
                skybox_path: String => eq, assign;
                ambient_color: glam::Vec3 => eq_or_bytes, assign;
                ambient_intensity: f32 => eq_or_bytes, assign;
                ambient: crate::AmbientSettings => eq_or_bytes, assign;
                sky: crate::SkySettings => eq_or_bytes, assign;
                nav_settings: crate::nav_settings::NavMeshSettings => eq_or_bytes, assign;
                fog: crate::FogSettings => eq_or_bytes, assign;
                layers: crate::LayerRegistry => eq_or_bytes, assign;
                collision_matrix: crate::CollisionMatrix => eq_or_bytes, assign;
                materials: crate::revision::Revisioned<std::collections::BTreeMap<String, crate::MaterialAsset>>
                    => eq_or_bytes, assign;
                meshes: crate::mesh_library::MeshLibrary => eq, put_meshes;
                // Placement only: compared and put back around the baked payload.
                probes: crate::lighting::probe::ProbeVolume => same_probes, put_probes;
                reflection_probes: crate::lighting::reflection_probe::ReflectionProbeSet
                    => same_reflections, put_reflections;
                lighting_settings: crate::lighting::LightingSettings => eq_or_bytes, assign;
            }
            $($rest)*
        }
    };
}
pub(crate) use authored_fields;

macro_rules! scene_state {
    (
        authored { $($authored:ident: $ty:ty => $same:ident, $put:ident;)* }
        other { $($field:ident: $class:ident, $keyed:ident $(as [$($saved:literal),*])?;)* }
        dev { $($dev:ident: $dev_class:ident, $dev_keyed:ident;)* }
    ) => {
        const ALWAYS: &[Field] = &[
            $(Field {
                name: stringify!($authored),
                class: Class::Authored,
                keyed: Keyed::No,
                saved_as: &[],
            },)*
            $(Field {
                name: stringify!($field),
                class: Class::$class,
                keyed: Keyed::$keyed,
                saved_as: &[$($($saved),*)?],
            },)*
        ];

        #[cfg(feature = "dev")]
        const DEV: &[Field] = &[$(Field {
            name: stringify!($dev),
            class: Class::$dev_class,
            keyed: Keyed::$dev_keyed,
            saved_as: &[],
        }),*];

        // No `..`: a field of `Scene` missing here, or listed and gone, fails to compile.
        const _: fn(&Scene) = |scene| {
            let Scene {
                $($authored: _,)*
                $($field: _,)*
                $(#[cfg(feature = "dev")] $dev: _,)*
            } = scene;
        };
    };
}

authored_fields!(scene_state {
    other {
        id: Identity, No;
        world: Entities, No as ["entities", "next_entity_id"];
        selected_entity_id: Selection, Dropped;
        decals: EditRuntime, Restored;
        lightmaps: BakeOutput, Restored;
        // Files beside the scene, not document fields: each saves with the field it names.
        occlusion: BakeOutput, Checked as ["lightmaps"];
        navmesh: BakeOutput, No as ["nav_settings"];
        pending_destroy: Runtime, Dropped;
        playing: Runtime, No;
        loads: Runtime, No;
        persistent: Runtime, Dropped;
        debug_lines: Runtime, No;
        shader_overrides: Runtime, Dropped;
        ragdolls: Runtime, Dropped;
        // A stale count only cuts a view once more.
        history_cuts: Frame, Dropped;
        shader_time: Frame, No;
        ui_time: Frame, No;
        inspected_reflection_probe: EditRuntime, No;
        mesh_reads: Follow, No;
        disk_text: DiskRecord, No;
        bake_files: DiskRecord, No;
        history: History, No;
        saves: DiskRecord, No;
    }
    dev {
        screen_exposure: Frame, No;
        lighting_bake_running: Frame, No;
        lighting_bake_requested: Request, No;
        lighting_bake_status: Frame, No;
        lighting_queue_requested: Request, No;
        lighting_queue: Frame, No;
    }
});

pub fn fields() -> Vec<Field> {
    let fields = ALWAYS.iter();
    #[cfg(feature = "dev")]
    let fields = fields.chain(DEV);
    fields.copied().collect()
}

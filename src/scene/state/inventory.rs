//! The inventory itself (#1649): every field of [`Scene`] and its [`Class`].
//!
//! The [`Class::Authored`] fields are listed once, in `authored_fields!`, with
//! their types and Undo's compare / put-back hooks, and that list is handed to
//! whoever asks for it: Undo's scene level is generated from it
//! (`authored_fields!(scene_level {})` in `undo::level`), and so is this file's
//! table. Every other field is listed in the `other` block below. The whole is one
//! exhaustive pattern over `Scene` (no `..`), so a field added to `Scene` and not
//! here, or one listed here and gone from `Scene`, fails to compile. The dev-only
//! fields sit in their own block, compiled with the `dev` feature.
//!
//! A field a lifecycle handles by a hook of its own says so beside its entry:
//! the hook belongs to the field, not to each lifecycle that touches it.

#[cfg(test)]
use super::{Class, Field, Keyed};
use crate::scene::Scene;

/// Hand the authored fields to `$then!` as `authored { field: Type => same, put; … }`,
/// followed by `$rest`. `same(&a, &b)` says whether a value is unchanged, as an
/// edit sees it; `put(value, &mut slot)` puts a recorded value back (both in
/// `undo::level`, where Undo expands the list).
macro_rules! authored_fields {
    ($then:ident { $($rest:tt)* }) => {
        $then! {
            authored {
                skybox_path: String => eq, assign;
                ambient_color: glam::Vec3 => eq_or_bytes, assign;
                ambient_intensity: f32 => eq_or_bytes, assign;
                sky: crate::scene::SkySettings => eq_or_bytes, assign;
                nav_settings: crate::scene::nav_settings::NavMeshSettings => eq_or_bytes, assign;
                fog: crate::scene::FogSettings => eq_or_bytes, assign;
                layers: crate::scene::LayerRegistry => eq_or_bytes, assign;
                collision_matrix: crate::scene::CollisionMatrix => eq_or_bytes, assign;
                materials: std::collections::BTreeMap<String, crate::scene::MaterialAsset>
                    => eq_or_bytes, assign;
                // Placement around bake output: compared and put back without the
                // baked SH, which the sidecar saves and Stop's snapshot carries.
                probes: crate::scene::lighting::probe::ProbeVolume => same_probes, put_probes;
                // The same split: the live cubemap paths stay.
                reflection_probes: crate::scene::lighting::reflection_probe::ReflectionProbeSet
                    => same_reflections, put_reflections;
                lighting_settings: crate::scene::lighting::LightingSettings => eq_or_bytes, assign;
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
        #[cfg(test)]
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

        #[cfg(all(test, feature = "dev"))]
        const DEV: &[Field] = &[$(Field {
            name: stringify!($dev),
            class: Class::$dev_class,
            keyed: Keyed::$dev_keyed,
            saved_as: &[],
        }),*];

        // Names every field of `Scene`, with no `..`: a field missing from the
        // inventory, or listed and gone, is a compile error here.
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
        // The scene's runtime identity; Stop's snapshot puts it back (#1374).
        id: Identity, No;
        // The entities, journaled for Undo (`ecs::journal`); saved as the document's
        // entities and its id allocator.
        world: Entities, No as ["entities", "next_entity_id"];
        // Dropped from the selection by the despawn door; Undo never selects.
        selected_entity_id: Selection, Dropped;
        // Stamped in edit mode they survive Play (`DecalSet::restore`); a load drops
        // them, a Play load carrying the persistent ones (`take_persistent_decals`).
        decals: EditRuntime, No;
        // An Undo of a Destroy puts its entity's entry back (`undo::lightmaps`).
        lightmaps: BakeOutput, Restored;
        pending_destroy: Runtime, Dropped;
        playing: Runtime, No;
        pending_load: Runtime, No;
        persistent: Runtime, Dropped;
        // Every document applied clears them (`serialize::apply`).
        shader_overrides: Runtime, Dropped;
        shader_time: Frame, No;
        ui_time: Frame, No;
        meshes_read_at: Follow, No;
        meshes_reimported: Follow, No;
        disk_text: DiskRecord, No;
        history: History, No;
    }
    dev {
        screen_exposure: Frame, No;
        lighting_bake_running: Frame, No;
    }
});

/// Every field of `Scene` in this build, classified.
#[cfg(test)]
pub fn fields() -> Vec<Field> {
    let fields = ALWAYS.iter();
    #[cfg(feature = "dev")]
    let fields = fields.chain(DEV);
    fields.copied().collect()
}

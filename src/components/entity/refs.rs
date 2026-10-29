//! src/components/entity/refs.rs — the one walk over every entity-id field (#1264).
//!
//! An entity names others by id in a handful of component fields. Every follower of
//! those references goes through this walk, never a list of its own: a prefab save,
//! stamp or propagation remapping them ([`EntityRefs::remap_refs`]), a despawn clearing
//! the ones that name the dead id (`ecs::World::forget_references`), a save keeping a
//! bone by name (`scene::skeleton::bone_refs`), and the property test that checks no
//! dead id survives (`scene::skeleton`'s `despawn_refs_tests`).
//!
//! The components that hold ids are listed once, in [`with_ref_components`]; each
//! implements [`EntityRefs`]. The leaves, as JSON pointers under the entity:
//! - `/selectable/target_graphic`, `/selectable/select_on/<0..3>`;
//! - `/joint/connected_body`;
//! - `/particles/sub_emitters/{birth,death,collision}`;
//! - `/lod_group/levels/<n>/renderers/<m>`;
//! - `/rect_transform/world_anchor/target`;
//! - `/mesh/skeleton/bodies/<bone>/joint/connected_body`: a saved ragdoll joint, in a
//!   document only (a load hands it to the bone, `scene::skeleton`'s `sync`);
//! - `/prefab_link/overrides/<escaped pointer>`: an id a prefab instance's overrides
//!   hold, at a modification's leaf, inside one's whole value or inside an added
//!   component (`prefab_link`'s `visit_override_refs`).
//!
//! Not walked: `MeshComponent`'s `skeleton.bones`, one `Option` per joint slot
//! (runtime-only, never saved, re-bound by name on load). They join the clearing
//! pass only: `MeshComponent` overrides [`EntityRefs::forget`] and
//! [`EntityRefs::names_any`], so a despawn or a load that leaves a bone behind sets
//! its slot to `None`, missing (#1273). Nor the runtime-only `OffMeshLinkData::owner`
//! and `SubEmitEvent::target`, never saved and read through `World::contains`.
//! A new component field that holds an entity id joins its component's
//! `EntityRefs` impl; a new component holding one also joins the list.

use super::Entity;
use crate::components::{
    JointComponent, LodGroupComponent, MeshComponent, RectTransformComponent, SelectableComponent,
    SubEmitters,
};

/// Visits one id slot: its JSON pointer and the slot. Setting it to `None` drops the
/// reference, which is that field's own `None` (a list drops the element).
pub type RefVisit<'a> = dyn FnMut(&str, &mut Option<u32>) + 'a;

/// A component that names other entities by id. See the module docs.
pub trait EntityRefs {
    /// Visit every id slot, with its JSON pointer inside the component. An optional
    /// slot is visited even when empty, so a caller can fill it.
    fn visit_refs(&mut self, visit: &mut RefVisit<'_>);

    /// Every id the component holds, read-only, with its pointer.
    fn each_ref(&self, visit: &mut dyn FnMut(&str, u32));

    /// Rewrite every reference through `map` (one `map` drops is cleared): how the
    /// references follow their entities when a prefab is saved, stamped or
    /// propagated, and when a load moves ids.
    fn remap_refs(&mut self, map: &dyn Fn(u32) -> Option<u32>) {
        self.visit_refs(&mut |_, r| *r = r.and_then(map));
    }

    /// Clear every reference to an entity that was destroyed (`dead` says which): a
    /// despawn, or a scene load that did not carry it. Unlike a remap dropping a
    /// reference, the target is gone, which a component may record (a marker hides).
    fn forget(&mut self, dead: &dyn Fn(u32) -> bool) {
        self.visit_refs(&mut |_, r| {
            if r.is_some_and(dead) {
                *r = None;
            }
        });
    }

    /// Whether any reference names an entity `dead` holds for.
    fn names_any(&self, dead: &dyn Fn(u32) -> bool) -> bool {
        let mut hit = false;
        self.each_ref(&mut |_, r| hit |= dead(r));
        hit
    }

    /// A value with at least one id slot of each kind it can hold, for the tests that
    /// fill every slot through [`EntityRefs::visit_refs`].
    #[cfg(test)]
    fn every_ref() -> Self
    where
        Self: Sized;
}

/// Calls `$m!` with the one list of components holding entity ids, as
/// `entity_field: Type` (the field is also the leaf pointer's first token).
macro_rules! with_ref_components {
    ($m:ident) => {
        $m! {
            selectable: $crate::components::SelectableComponent,
            joint: $crate::components::JointComponent,
            particles: $crate::components::ParticleEmitterComponent,
            lod_group: $crate::components::LodGroupComponent,
            mesh: $crate::components::MeshComponent,
            rect_transform: $crate::components::RectTransformComponent,
            prefab_link: $crate::components::PrefabLink,
        }
    };
}
pub(crate) use with_ref_components;

macro_rules! entity_walk {
    ($($field:ident: $ty:ty,)*) => {
        /// An entity's id slots are its components', each pointer under the entity
        /// (see the module docs).
        impl EntityRefs for Entity {
            fn visit_refs(&mut self, visit: &mut RefVisit<'_>) {
                $(if let Some(c) = &mut self.$field {
                    let prefix = concat!("/", stringify!($field));
                    c.visit_refs(&mut |p, r| visit(&format!("{prefix}{p}"), r));
                })*
            }

            fn each_ref(&self, visit: &mut dyn FnMut(&str, u32)) {
                $(if let Some(c) = &self.$field {
                    let prefix = concat!("/", stringify!($field));
                    c.each_ref(&mut |p, r| visit(&format!("{prefix}{p}"), r));
                })*
            }

            fn forget(&mut self, dead: &dyn Fn(u32) -> bool) {
                $(if let Some(c) = &mut self.$field {
                    c.forget(dead);
                })*
            }

            /// An entity carrying every reference-holding component, each with
            /// [`EntityRefs::every_ref`].
            #[cfg(test)]
            fn every_ref() -> Self {
                let mut e = Self::new(0, "Refs".into());
                $(e.$field = Some(<$ty as EntityRefs>::every_ref());)*
                e
            }
        }
    };
}
with_ref_components!(entity_walk);

impl Entity {
    /// Whether `pointer` (a JSON pointer into the entity document, as prefab
    /// overrides key their leaves) names one of the component id leaves. The ids an
    /// instance's overrides hold are visited under `/prefab_link/overrides/…`, which
    /// is not one.
    pub fn is_ref_pointer(pointer: &str) -> bool {
        let under = |prefix: &str, refs: &[&str]| {
            pointer
                .strip_prefix(prefix)
                .is_some_and(|rest| refs.contains(&rest))
        };
        under("/selectable", &SelectableComponent::REF_POINTERS)
            || under("/joint", &JointComponent::REF_POINTERS)
            || under("/particles", &SubEmitters::REF_POINTERS)
            || pointer
                .strip_prefix("/lod_group")
                .is_some_and(LodGroupComponent::is_ref_pointer)
            || pointer
                .strip_prefix("/mesh")
                .is_some_and(MeshComponent::is_ref_pointer)
            || under("/rect_transform", &RectTransformComponent::REF_POINTERS)
    }
}

impl Entity {
    /// Whether `pointer` names an element of a *list* of ids (`Vec<u32>`, a LOD
    /// level's renderers), where a dropped reference leaves the list rather than
    /// becoming the slot's `None`.
    pub fn is_ref_list_pointer(pointer: &str) -> bool {
        pointer
            .strip_prefix("/lod_group")
            .is_some_and(LodGroupComponent::is_ref_pointer)
    }
}

/// Escape one JSON-pointer reference token (RFC 6901: `~` → `~0`, `/` → `~1`): the
/// one escape every pointer builder uses (prefab overrides' too).
pub fn escape_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

//! The one walk over every entity-id field; every follower of references goes through it.

use super::Entity;
use crate::{
    JointComponent, LodGroupComponent, MeshComponent, RectTransformComponent, SelectableComponent,
    SubEmitters,
};

/// Setting a slot to `None` drops the reference.
pub type RefVisit<'a> = dyn FnMut(&str, &mut Option<u32>) + 'a;

pub trait EntityRefs {
    /// Visits an optional slot even when empty, so a caller can fill it.
    fn visit_refs(&mut self, visit: &mut RefVisit<'_>);

    fn each_ref(&self, visit: &mut dyn FnMut(&str, u32));

    fn remap_refs(&mut self, map: &dyn Fn(u32) -> Option<u32>) {
        self.visit_refs(&mut |_, r| *r = r.and_then(map));
    }

    fn forget(&mut self, dead: &dyn Fn(u32) -> bool) {
        self.visit_refs(&mut |_, r| {
            if r.is_some_and(dead) {
                *r = None;
            }
        });
    }

    /// Includes clearing-only slots (a skinned mesh's bones): what the World's reverse index keys.
    fn each_held(&self, visit: &mut dyn FnMut(u32)) {
        self.each_ref(&mut |_, r| visit(r));
    }

    /// Test hook: every kind of id slot filled.
    #[doc(hidden)]
    fn every_ref() -> Self
    where
        Self: Sized;
}

/// A new id-holding component joins here and `Entity::is_ref_pointer`.
#[macro_export]
macro_rules! with_ref_components {
    ($m:ident) => {
        $m! {
            selectable: $crate::SelectableComponent,
            joint: $crate::JointComponent,
            particles: $crate::ParticleEmitterComponent,
            lod_group: $crate::LodGroupComponent,
            mesh: $crate::MeshComponent,
            rect_transform: $crate::RectTransformComponent,
            prefab_link: $crate::PrefabLink,
        }
    };
}

macro_rules! entity_walk {
    ($($field:ident: $ty:ty,)*) => {
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

            #[doc(hidden)]
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
    /// An id under `/prefab_link/overrides` is not a component leaf.
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
    /// A dropped id leaves this list instead of becoming `None`.
    pub fn is_ref_list_pointer(pointer: &str) -> bool {
        pointer
            .strip_prefix("/lod_group")
            .is_some_and(LodGroupComponent::is_ref_pointer)
    }
}

/// RFC 6901's escape, the one every pointer builder uses.
pub fn escape_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

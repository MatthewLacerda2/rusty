//! src/scene/authoring/components.rs — Add/Remove first-class components.
//!
//! The `ComponentKind` enum (the Add-Component menu's first-class kinds) plus the
//! shared `add_component` / `remove_component` verbs both the editor add-menu and
//! the `Scene.AddComponent` / `Scene.RemoveComponent` API route through, so the two
//! can never drift. Split out of `scene::authoring` to keep that module under the
//! size cap; re-exported from there so existing `authoring::ComponentKind` paths
//! still resolve.

use crate::components::registry::first_class;
use crate::components::FirstClass;
use crate::scene::authoring::defaults::attach_default_material;
use crate::scene::authoring::dependency;
use crate::scene::authoring::fields::KINDS;
use crate::scene::Scene;

/// Declares `ComponentKind`, `ComponentKind::ALL` and `ComponentKind::parse` from
/// the registry (`components::registry`), so a new kind is its row and `ALL` can
/// never miss a variant, carry a stale length, or drift out of declaration order
/// (#570). The order is the registry's.
macro_rules! component_kinds {
    ($($field:ident: $ty:ty => $kind:ident [$($acc:ident)*];)*) => {
        /// The first-class components the inspector's "Add Component" menu can
        /// attach / detach. `Script` is excluded: a script attachment carries a path
        /// (it is "add *which* script"), so it has its own verb rather than a
        /// defaulted add ([`add_script`](super::add_script), `Scene.AddScript`).
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum ComponentKind {
            $($kind),*
        }

        impl ComponentKind {
            /// Every first-class kind, in declaration order — for the dependency
            /// machinery's reverse lookups: finding a removed kind's dependents
            /// ([`dependency::remove_with_cascade`]) and reconciling/enforcing unmet
            /// requirements. Generated with the enum, so it is complete by
            /// construction.
            pub const ALL: &'static [ComponentKind] = &[$(Self::$kind),*];

            /// Parse an "Add Component" kind name (any case): the kind's own name,
            /// the name `Component.*` gives it, or one of its aliases
            /// ([`FirstClass`]), e.g. `RigidBody`, `Texture`, `MeshRenderer`.
            pub fn parse(name: &str) -> Option<Self> {
                let lower = name.to_lowercase();
                $(
                    if lower.eq_ignore_ascii_case(stringify!($kind))
                        || lower.eq_ignore_ascii_case(<$ty as FirstClass>::NAME)
                        || <$ty as FirstClass>::ALIASES.contains(&lower.as_str())
                    {
                        return Some(Self::$kind);
                    }
                )*
                None
            }
        }
    };
}

first_class!(component_kinds);

impl ComponentKind {
    /// The first-class components this kind depends on — rusty's `RequireComponent`
    /// (Unity's `[RequireComponent(typeof(T))]`). The single declaration every
    /// add/remove/load surface consults: adding a kind auto-adds these if missing,
    /// removing one of these cascades to the dependents that declare it. Flat
    /// `kind → [kinds]`: `PostProcessing → Camera` (post-processing is inert
    /// without a camera to process), and the UI graphic and clip need a rect to fill
    /// or clip to, a Selectable a rect to be hit in, and the layout pair a rect to
    /// arrange or size (`Image` / `Text` / `Shape` / `RectMask` / `Mask` /
    /// `BackdropFilter` / `Selectable` / `LayoutGroup` / `LayoutElement →
    /// RectTransform`, as Unity's `Graphic`, `RectMask2D`, `Mask` and
    /// layout components require one), and a Joint the Rigidbody it constrains
    /// (`Joint → RigidBody`, as Unity's `Joint`). A new dependency is one line here, enforced
    /// everywhere by construction.
    pub fn requires(self) -> &'static [ComponentKind] {
        match self {
            ComponentKind::PostProcessing => &[ComponentKind::Camera],
            ComponentKind::Image
            | ComponentKind::Text
            | ComponentKind::Shape
            | ComponentKind::RectMask
            | ComponentKind::Mask
            | ComponentKind::BackdropFilter
            | ComponentKind::Selectable
            | ComponentKind::LayoutGroup
            | ComponentKind::LayoutElement => &[ComponentKind::RectTransform],
            ComponentKind::Joint => &[ComponentKind::RigidBody],
            _ => &[],
        }
    }

    /// [`parse`](Self::parse), with an unknown name an error that lists every kind
    /// by the name `Component.*` gives it (`Material`, `AudioSource`, `LODGroup`), so
    /// the two errors list the same names: the message `Scene.AddComponent` and
    /// `Scene.RemoveComponent` raise.
    pub fn parse_or_list(name: &str) -> Result<Self, String> {
        Self::parse(name).ok_or_else(|| {
            let kinds: Vec<&str> = KINDS
                .iter()
                .filter(|k| k.add.is_some())
                .map(|k| k.name)
                .collect();
            format!("unknown component '{name}' (kinds: {})", kinds.join(", "))
        })
    }
}

/// Attach a first-class component of `kind` to entity `id` with the inspector's
/// default values, replacing any existing one (the API is idempotent-by-replace), and
/// auto-adding any component `kind` requires (Unity's `RequireComponent`, via
/// [`ComponentKind::requires`]). Returns `false` if the entity is missing. The
/// inspector add-menu and the `Scene.AddComponent` API both route here.
pub fn add_component(scene: &mut Scene, id: u32, kind: ComponentKind) -> bool {
    // Material attaches a reference AND creates a shared library asset (it touches
    // `scene.materials`, not just the entity guard), so it routes off on its own; it
    // has no requirements, so no dependency work follows.
    if kind == ComponentKind::Texture {
        return attach_default_material(scene, id);
    }
    dependency::add_with_requirements(&mut scene.world, id, kind)
}

/// Detach a first-class component of `kind` from entity `id`, cascading to every
/// dependent that requires it (e.g. removing `Camera` drops the camera-only
/// `PostProcessing` stack) — driven by [`ComponentKind::requires`], not hand-written
/// here. Returns `false` if the entity does not exist (clearing an absent component is
/// otherwise a no-op success).
pub fn remove_component(scene: &mut Scene, id: u32, kind: ComponentKind) -> bool {
    dependency::remove_with_cascade(&mut scene.world, id, kind)
}

#[cfg(test)]
#[path = "components_tests.rs"]
mod tests;

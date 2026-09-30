//! src/scene/authoring/components.rs — Add/Remove first-class components.
//!
//! The `ComponentKind` enum (the Add-Component menu's first-class kinds) plus the
//! shared `add_component` / `remove_component` verbs both the editor add-menu and
//! the `Scene.AddComponent` / `Scene.RemoveComponent` API route through, so the two
//! can never drift. Split out of `scene::authoring` to keep that module under the
//! size cap; re-exported from there so existing `authoring::ComponentKind` paths
//! still resolve.
//!
//! Allowed deps: components, scene.

use crate::scene::authoring::defaults::attach_default_material;
use crate::scene::authoring::dependency;
use crate::scene::Scene;

/// Declares `ComponentKind` and `ComponentKind::ALL` from one list, so a new kind is
/// one line and `ALL` can never miss a variant, carry a stale length, or drift out of
/// declaration order (#570).
macro_rules! component_kinds {
    ($(#[$meta:meta])* $vis:vis enum $name:ident { $($variant:ident),+ $(,)? }) => {
        $(#[$meta])*
        $vis enum $name {
            $($variant),+
        }

        impl $name {
            /// Every first-class kind, in declaration order (the Add Component menu
            /// and inspector order) — for the dependency machinery's reverse lookups:
            /// finding a removed kind's dependents ([`dependency::remove_with_cascade`])
            /// and reconciling/enforcing unmet requirements. Generated with the enum,
            /// so it is complete by construction.
            pub const ALL: &'static [$name] = &[$(Self::$variant),+];
        }
    };
}

component_kinds! {
    /// The first-class components the inspector's "Add Component" menu can attach /
    /// detach. `Script` is excluded: a script attachment carries a path (it is "add
    /// *which* script"), so it has its own API verb rather than a defaulted add.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ComponentKind {
        Light,
        Animator,
        Collider,
        RigidBody,
        Texture,
        NavMeshAgent,
        Camera,
        Particles,
        VisualCorrection,
        Audio,
        Canvas,
        RectTransform,
        Image,
        CanvasGroup,
        RectMask,
        Text,
        Selectable,
        LayoutGroup,
        LayoutElement,
        Joint,
        LodGroup,
    }
}

impl ComponentKind {
    /// The first-class components this kind depends on — rusty's `RequireComponent`
    /// (Unity's `[RequireComponent(typeof(T))]`). The single declaration every
    /// add/remove/load surface consults: adding a kind auto-adds these if missing,
    /// removing one of these cascades to the dependents that declare it. Flat
    /// `kind → [kinds]`: `VisualCorrection → Camera` (a correction stack is inert
    /// without a camera to correct), and the UI graphic and clip need a rect to fill
    /// or clip to, a Selectable a rect to be hit in, and the layout pair a rect to
    /// arrange or size (`Image` / `Text` / `RectMask` / `Selectable` / `LayoutGroup` /
    /// `LayoutElement → RectTransform`, as Unity's `Graphic`, `RectMask2D` and
    /// layout components require one), and a Joint the Rigidbody it constrains
    /// (`Joint → RigidBody`, as Unity's `Joint`). A new dependency is one line here, enforced
    /// everywhere by construction.
    pub fn requires(self) -> &'static [ComponentKind] {
        match self {
            ComponentKind::VisualCorrection => &[ComponentKind::Camera],
            ComponentKind::Image
            | ComponentKind::Text
            | ComponentKind::RectMask
            | ComponentKind::Selectable
            | ComponentKind::LayoutGroup
            | ComponentKind::LayoutElement => &[ComponentKind::RectTransform],
            ComponentKind::Joint => &[ComponentKind::RigidBody],
            _ => &[],
        }
    }

    /// Parse an "Add Component" kind name (case-insensitive). Accepts the short
    /// names the menu uses (e.g. `RigidBody`, `Texture`, `NavMeshAgent`).
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "light" => Some(Self::Light),
            "animator" => Some(Self::Animator),
            "collider" => Some(Self::Collider),
            "rigidbody" => Some(Self::RigidBody),
            "texture" | "material" => Some(Self::Texture),
            "navmeshagent" | "navagent" => Some(Self::NavMeshAgent),
            "camera" => Some(Self::Camera),
            "particles" | "particlesystem" => Some(Self::Particles),
            "visualcorrection" => Some(Self::VisualCorrection),
            "audio" | "audiosource" => Some(Self::Audio),
            "canvas" => Some(Self::Canvas),
            "recttransform" => Some(Self::RectTransform),
            "image" => Some(Self::Image),
            "canvasgroup" => Some(Self::CanvasGroup),
            "rectmask" | "rectmask2d" => Some(Self::RectMask),
            "text" | "textmeshpro" | "textmeshprougui" => Some(Self::Text),
            "selectable" => Some(Self::Selectable),
            "layoutgroup" => Some(Self::LayoutGroup),
            "layoutelement" | "contentsizefitter" => Some(Self::LayoutElement),
            "joint" | "fixedjoint" | "hingejoint" | "characterjoint" => Some(Self::Joint),
            "lodgroup" | "lod" => Some(Self::LodGroup),
            _ => None,
        }
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
/// `VisualCorrection` stack) — driven by [`ComponentKind::requires`], not hand-written
/// here. Returns `false` if the entity does not exist (clearing an absent component is
/// otherwise a no-op success).
pub fn remove_component(scene: &mut Scene, id: u32, kind: ComponentKind) -> bool {
    dependency::remove_with_cascade(&mut scene.world, id, kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::authoring::create_entity;

    #[test]
    fn add_remove_component_with_cascades() {
        let mut scene = Scene::new();
        let id = create_entity(&mut scene, "Mob", None);
        assert!(add_component(&mut scene, id, ComponentKind::Light));
        assert!(scene.world.has_light(id));
        assert!(remove_component(&mut scene, id, ComponentKind::Light));
        assert!(!scene.world.has_light(id));

        // Adding VisualCorrection auto-adds its required Camera (RequireComponent).
        add_component(&mut scene, id, ComponentKind::VisualCorrection);
        assert!(scene.world.has_visual_correction(id) && scene.world.has_camera(id));

        // Removing Camera cascades to VisualCorrection (editor parity).
        remove_component(&mut scene, id, ComponentKind::Camera);
        assert!(!scene.world.has_camera(id) && !scene.world.has_visual_correction(id));

        assert!(!add_component(&mut scene, 9999, ComponentKind::Light));
    }

    #[test]
    fn add_remove_audio_source() {
        let mut scene = Scene::new();
        let id = create_entity(&mut scene, "Speaker", None);
        assert!(add_component(&mut scene, id, ComponentKind::Audio));
        assert!(scene.world.has_audio(id));
        assert!(remove_component(&mut scene, id, ComponentKind::Audio));
        assert!(!scene.world.has_audio(id));
    }

    #[test]
    fn all_lists_every_kind_once_in_declaration_order() {
        // `ALL[i]`'s discriminant is `i`: no gap, duplicate or reorder. Every kind also
        // parses from its own name, so `parse` cannot miss a variant either.
        for (i, &kind) in ComponentKind::ALL.iter().enumerate() {
            assert_eq!(kind as usize, i, "{kind:?} out of place in ALL");
            assert_eq!(ComponentKind::parse(&format!("{kind:?}")), Some(kind));
        }
    }

    #[test]
    fn parse_names_are_case_insensitive() {
        assert_eq!(
            ComponentKind::parse("material"),
            Some(ComponentKind::Texture)
        );
        assert_eq!(ComponentKind::parse("AUDIO"), Some(ComponentKind::Audio));
        assert_eq!(
            ComponentKind::parse("AudioSource"),
            Some(ComponentKind::Audio)
        );
        assert_eq!(ComponentKind::parse("nope"), None);
    }
}

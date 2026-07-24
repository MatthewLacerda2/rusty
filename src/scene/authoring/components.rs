//! src/scene/authoring_components.rs — Add/Remove first-class components.
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
}

impl ComponentKind {
    /// Every first-class kind, for the dependency machinery's reverse lookups —
    /// finding a removed kind's dependents ([`dependency::remove_with_cascade`]) and
    /// reconciling/enforcing unmet requirements. The per-kind matches in `dependency`
    /// are compiler-checked exhaustive; a test guards this list against drift.
    pub const ALL: [ComponentKind; 10] = [
        Self::Light,
        Self::Animator,
        Self::Collider,
        Self::RigidBody,
        Self::Texture,
        Self::NavMeshAgent,
        Self::Camera,
        Self::Particles,
        Self::VisualCorrection,
        Self::Audio,
    ];

    /// The first-class components this kind depends on — rusty's `RequireComponent`
    /// (Unity's `[RequireComponent(typeof(T))]`). The single declaration every
    /// add/remove/load surface consults: adding a kind auto-adds these if missing,
    /// removing one of these cascades to the dependents that declare it. Flat
    /// `kind → [kinds]`; `VisualCorrection → Camera` (a correction stack is inert
    /// without a camera to correct) is the sole entry today — a new dependency is one
    /// line here, enforced everywhere by construction.
    pub fn requires(self) -> &'static [ComponentKind] {
        match self {
            ComponentKind::VisualCorrection => &[ComponentKind::Camera],
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

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

use crate::scene::authoring::defaults::{
    attach_default_material, default_animator, default_camera, default_collider, default_light,
    default_nav_agent, default_rigidbody, default_visual_correction,
};
use crate::scene::{AudioSourceComponent, ParticleEmitterComponent, Scene};

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
/// default values, replacing any existing one (editor offers it only when absent;
/// the API is idempotent-by-replace). Returns `false` if the entity is missing. The
/// inspector add-menu and the `Scene.AddComponent` API both route here.
pub fn add_component(scene: &mut Scene, id: u32, kind: ComponentKind) -> bool {
    // Material attaches a reference AND creates a shared library asset (it touches
    // `scene.materials`, not just the entity guard), so it routes off on its own.
    if kind == ComponentKind::Texture {
        return attach_default_material(scene, id);
    }
    let w = &mut scene.world;
    match kind {
        ComponentKind::Light => w.set_light(id, Some(default_light())),
        ComponentKind::Animator => w.set_animator(id, Some(default_animator())),
        ComponentKind::Collider => w.set_collider(id, Some(default_collider())),
        ComponentKind::RigidBody => w.set_rigidbody(id, Some(default_rigidbody())),
        ComponentKind::Texture => unreachable!("handled above"),
        ComponentKind::NavMeshAgent => w.set_nav_agent(id, Some(default_nav_agent())),
        ComponentKind::Camera => w.set_camera(id, Some(default_camera())),
        ComponentKind::Particles => w.set_particles(id, Some(ParticleEmitterComponent::default())),
        ComponentKind::VisualCorrection => {
            w.set_visual_correction(id, Some(default_visual_correction()))
        }
        ComponentKind::Audio => w.set_audio(id, Some(AudioSourceComponent::default())),
    }
}

/// Detach a first-class component of `kind` from entity `id`, applying the same
/// cascade the editor's inspector enforces: removing `Camera` also drops the
/// camera-only `VisualCorrection` stack. Returns `false` if the entity does not
/// exist (clearing an absent component is otherwise a no-op success).
pub fn remove_component(scene: &mut Scene, id: u32, kind: ComponentKind) -> bool {
    let w = &mut scene.world;
    match kind {
        ComponentKind::Light => w.set_light(id, None),
        ComponentKind::Animator => w.set_animator(id, None),
        ComponentKind::Collider => w.set_collider(id, None),
        ComponentKind::RigidBody => w.set_rigidbody(id, None),
        // Drop only the reference; the shared library material may still be in use.
        ComponentKind::Texture => w.set_material(id, None),
        ComponentKind::NavMeshAgent => w.set_nav_agent(id, None),
        ComponentKind::Camera => {
            w.set_visual_correction(id, None);
            w.set_camera(id, None)
        }
        ComponentKind::Particles => w.set_particles(id, None),
        ComponentKind::VisualCorrection => w.set_visual_correction(id, None),
        ComponentKind::Audio => w.set_audio(id, None),
    }
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

        // Removing Camera cascades to VisualCorrection (editor parity).
        add_component(&mut scene, id, ComponentKind::Camera);
        add_component(&mut scene, id, ComponentKind::VisualCorrection);
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

//! src/scene/authoring/dependency.rs — Component dependency mechanism (#359).
//!
//! rusty's `RequireComponent` (Unity's `[RequireComponent(typeof(T))]`), promoted
//! from the hand-rolled Camera↔VisualCorrection one-off into a declared rule enforced
//! everywhere. [`ComponentKind::requires`] is the single declaration; the verbs here
//! are the single enforcement path the editor Add menu, the
//! `Scene.AddComponent`/`RemoveComponent` API, and scene load all route through, so no
//! surface re-implements the rule:
//!
//!   - **add** ([`add_with_requirements`]) auto-adds a kind's missing requirements
//!     (with defaults), recursively;
//!   - **remove** ([`remove_with_cascade`]) cascades to the dependents that require the
//!     removed kind;
//!   - **reconcile** ([`reconcile_requirements`]) — the editor's per-frame net — drops
//!     any component whose requirements are unmet, the reactive form of the cascade;
//!   - **enforce-on-load** ([`enforce_requirements`]) validates a loaded scene,
//!     auto-adding missing requirements (same rule as add) and reporting what it added
//!     so the caller can warn.
//!
//! All verbs operate at the World level (every dependency-relevant kind is a World
//! component), so the editor (which holds only `&mut World`) and the scene-level
//! `add_component` share one path. Material is the one add that also creates a library
//! asset; it has no requirements and is required by nothing, so it never flows through
//! here — [`set_default`]'s Material arm exists only for completeness and stages the
//! default asset the same way the editor Add menu does.
//!
//! Allowed deps: components, scene, ecs.

use crate::ecs::World;
use crate::scene::authoring::components::ComponentKind;
use crate::scene::authoring::defaults::{
    default_animator, default_camera, default_collider, default_light, default_material,
    default_nav_agent, default_rigidbody, default_visual_correction,
};
use crate::scene::{AudioSourceComponent, MaterialComponent, ParticleEmitterComponent};

/// Does entity `id` carry a component of `kind`? The dependency machinery's presence
/// probe, one arm per first-class kind.
pub(crate) fn has_kind(world: &World, id: u32, kind: ComponentKind) -> bool {
    match kind {
        ComponentKind::Light => world.has_light(id),
        ComponentKind::Animator => world.has_animator(id),
        ComponentKind::Collider => world.has_collider(id),
        ComponentKind::RigidBody => world.has_rigidbody(id),
        ComponentKind::Texture => world.has_material(id),
        ComponentKind::NavMeshAgent => world.has_nav_agent(id),
        ComponentKind::Camera => world.has_camera(id),
        ComponentKind::Particles => world.has_particles(id),
        ComponentKind::VisualCorrection => world.has_visual_correction(id),
        ComponentKind::Audio => world.has_audio(id),
    }
}

/// Attach `kind` with its Add-Component defaults at the World level, returning `false`
/// when the entity is missing. Material rides in as a staged pending asset (folded into
/// the library when the world guard drops) — the same path the editor Add menu uses.
pub(crate) fn set_default(world: &mut World, id: u32, kind: ComponentKind) -> bool {
    match kind {
        ComponentKind::Light => world.set_light(id, Some(default_light())),
        ComponentKind::Animator => world.set_animator(id, Some(default_animator())),
        ComponentKind::Collider => world.set_collider(id, Some(default_collider())),
        ComponentKind::RigidBody => world.set_rigidbody(id, Some(default_rigidbody())),
        ComponentKind::Texture => attach_default_material_world(world, id),
        ComponentKind::NavMeshAgent => world.set_nav_agent(id, Some(default_nav_agent())),
        ComponentKind::Camera => world.set_camera(id, Some(default_camera())),
        ComponentKind::Particles => {
            world.set_particles(id, Some(ParticleEmitterComponent::default()))
        }
        ComponentKind::VisualCorrection => {
            world.set_visual_correction(id, Some(default_visual_correction()))
        }
        ComponentKind::Audio => world.set_audio(id, Some(AudioSourceComponent::default())),
    }
}

/// Detach `kind` from entity `id` (raw, no cascade). Returns `false` when the entity is
/// missing (clearing an absent component is otherwise a no-op success).
pub(crate) fn clear_one(world: &mut World, id: u32, kind: ComponentKind) -> bool {
    match kind {
        ComponentKind::Light => world.set_light(id, None),
        ComponentKind::Animator => world.set_animator(id, None),
        ComponentKind::Collider => world.set_collider(id, None),
        ComponentKind::RigidBody => world.set_rigidbody(id, None),
        // Drop only the reference; the shared library material may still be in use.
        ComponentKind::Texture => world.set_material(id, None),
        ComponentKind::NavMeshAgent => world.set_nav_agent(id, None),
        ComponentKind::Camera => world.set_camera(id, None),
        ComponentKind::Particles => world.set_particles(id, None),
        ComponentKind::VisualCorrection => world.set_visual_correction(id, None),
        ComponentKind::Audio => world.set_audio(id, None),
    }
}

/// Attach a default per-entity library material at the World level via staging — the
/// editor Add menu's material path (the World holds no library, so the default asset
/// rides along as `pending_material`, folded in once the guard drops).
fn attach_default_material_world(world: &mut World, id: u32) -> bool {
    let key = format!("entity_{id}_material");
    if !world.set_material(id, Some(MaterialComponent { material: key })) {
        return false;
    }
    world.stage_pending_material(id, default_material());
    true
}

/// Auto-add every missing requirement of `kind`, recursively (RequireComponent on
/// add). Shared by every surface that adds a component.
pub fn satisfy_requirements(world: &mut World, id: u32, kind: ComponentKind) {
    for &req in kind.requires() {
        if !has_kind(world, id, req) {
            set_default(world, id, req);
            satisfy_requirements(world, id, req);
        }
    }
}

/// Add `kind` with defaults and auto-satisfy its requirements — the World-level "add a
/// component" the editor Add menu routes each dependency-bearing entry through. Returns
/// `false` when the entity is missing (nothing is added).
pub fn add_with_requirements(world: &mut World, id: u32, kind: ComponentKind) -> bool {
    if !set_default(world, id, kind) {
        return false;
    }
    satisfy_requirements(world, id, kind);
    true
}

/// Remove `kind` and cascade to every dependent that requires it, recursively — the
/// shipped Camera→VisualCorrection cascade, now driven by [`ComponentKind::requires`].
/// Returns `false` when the entity is missing.
pub fn remove_with_cascade(world: &mut World, id: u32, kind: ComponentKind) -> bool {
    for dependent in ComponentKind::ALL {
        if dependent.requires().contains(&kind) && has_kind(world, id, dependent) {
            remove_with_cascade(world, id, dependent);
        }
    }
    clear_one(world, id, kind)
}

/// Drop any component on `id` whose declared requirements are unmet, repeating until
/// stable (removing a dependent can orphan its own dependents). The editor's per-frame
/// safety net — the reactive form of the remove cascade, catching drift the add/remove
/// verbs didn't produce (a Camera cleared out from under a VisualCorrection by another
/// card). Returns `true` if it removed anything.
pub fn reconcile_requirements(world: &mut World, id: u32) -> bool {
    let mut changed = false;
    loop {
        let mut removed = false;
        for kind in ComponentKind::ALL {
            if !has_kind(world, id, kind) {
                continue;
            }
            if kind.requires().iter().any(|&req| !has_kind(world, id, req)) {
                clear_one(world, id, kind);
                removed = true;
            }
        }
        changed |= removed;
        if !removed {
            break;
        }
    }
    changed
}

/// Validate entity `id`'s declared requirements after a load, auto-adding any that are
/// missing (same rule as add). Returns `(dependent, requirement)` pairs for what it
/// added so the caller can warn — old/hand-edited scenes keep loading instead of
/// silently violating a dependency.
pub fn enforce_requirements(world: &mut World, id: u32) -> Vec<(ComponentKind, ComponentKind)> {
    let mut added = Vec::new();
    for kind in ComponentKind::ALL {
        if !has_kind(world, id, kind) {
            continue;
        }
        for &req in kind.requires() {
            if !has_kind(world, id, req) {
                set_default(world, id, req);
                added.push((kind, req));
            }
        }
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::authoring::create_entity;
    use crate::scene::Scene;

    #[test]
    fn all_kinds_are_addable_and_probeable() {
        // Guards `ComponentKind::ALL` against drift: every listed kind adds a default
        // and is then seen by the presence probe (the compiler already forces the
        // per-kind match arms to be exhaustive).
        let mut scene = Scene::new();
        for kind in ComponentKind::ALL {
            let id = create_entity(&mut scene, "E", None);
            assert!(set_default(&mut scene.world, id, kind), "{kind:?} adds");
            assert!(has_kind(&scene.world, id, kind), "{kind:?} is probeable");
        }
    }

    #[test]
    fn add_auto_satisfies_requirement() {
        let mut scene = Scene::new();
        let id = create_entity(&mut scene, "FX", None);
        assert!(add_with_requirements(
            &mut scene.world,
            id,
            ComponentKind::VisualCorrection
        ));
        assert!(scene.world.has_visual_correction(id));
        assert!(scene.world.has_camera(id), "requirement auto-added");
    }

    #[test]
    fn remove_cascades_to_dependents() {
        let mut scene = Scene::new();
        let id = create_entity(&mut scene, "FX", None);
        add_with_requirements(&mut scene.world, id, ComponentKind::VisualCorrection);
        assert!(remove_with_cascade(
            &mut scene.world,
            id,
            ComponentKind::Camera
        ));
        assert!(!scene.world.has_camera(id));
        assert!(!scene.world.has_visual_correction(id), "dependent cascaded");
    }

    #[test]
    fn reconcile_drops_orphaned_dependent() {
        let mut scene = Scene::new();
        let id = create_entity(&mut scene, "FX", None);
        add_with_requirements(&mut scene.world, id, ComponentKind::VisualCorrection);
        // Clear the requirement out from under the dependent (as another card might).
        scene.world.set_camera(id, None);
        assert!(reconcile_requirements(&mut scene.world, id));
        assert!(!scene.world.has_visual_correction(id));
    }

    #[test]
    fn enforce_on_load_auto_adds_missing_requirement() {
        let mut scene = Scene::new();
        let id = create_entity(&mut scene, "FX", None);
        // Simulate a hand-edited scene: a dependent with no requirement.
        scene
            .world
            .set_visual_correction(id, Some(default_visual_correction()));
        let added = enforce_requirements(&mut scene.world, id);
        assert_eq!(
            added,
            vec![(ComponentKind::VisualCorrection, ComponentKind::Camera)]
        );
        assert!(scene.world.has_camera(id));
        // Idempotent: a second pass finds nothing to add.
        assert!(enforce_requirements(&mut scene.world, id).is_empty());
    }
}

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
//! here — `set_default`'s Material arm exists only for completeness and stages the
//! default asset the same way the editor Add menu does.
//!
//! Allowed deps: components, scene, ecs.

use crate::components::{
    CanvasComponent, CanvasGroupComponent, ImageComponent, RectMaskComponent,
    RectTransformComponent, SelectableComponent, TextComponent,
};
use crate::components::{LayoutElementComponent, LayoutGroupComponent};
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
        ComponentKind::Canvas => world.has_canvas(id),
        ComponentKind::RectTransform => world.has_rect_transform(id),
        ComponentKind::Image => world.has_image(id),
        ComponentKind::CanvasGroup => world.has_canvas_group(id),
        ComponentKind::RectMask => world.has_rect_mask(id),
        ComponentKind::Text => world.has_text(id),
        ComponentKind::Selectable => world.has_selectable(id),
        ComponentKind::LayoutGroup => world.has_layout_group(id),
        ComponentKind::LayoutElement => world.has_layout_element(id),
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
        ComponentKind::Canvas => world.set_canvas(id, Some(CanvasComponent::default())),
        ComponentKind::RectTransform => {
            world.set_rect_transform(id, Some(RectTransformComponent::default()))
        }
        ComponentKind::Image => world.set_image(id, Some(ImageComponent::default())),
        ComponentKind::CanvasGroup => {
            world.set_canvas_group(id, Some(CanvasGroupComponent::default()))
        }
        ComponentKind::RectMask => world.set_rect_mask(id, Some(RectMaskComponent::default())),
        ComponentKind::Text => world.set_text(id, Some(TextComponent::default())),
        ComponentKind::Selectable => world.set_selectable(id, Some(SelectableComponent::default())),
        ComponentKind::LayoutGroup => {
            world.set_layout_group(id, Some(LayoutGroupComponent::default()))
        }
        ComponentKind::LayoutElement => {
            world.set_layout_element(id, Some(LayoutElementComponent::default()))
        }
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
        ComponentKind::Canvas => world.set_canvas(id, None),
        ComponentKind::RectTransform => world.set_rect_transform(id, None),
        ComponentKind::Image => world.set_image(id, None),
        ComponentKind::CanvasGroup => world.set_canvas_group(id, None),
        ComponentKind::RectMask => world.set_rect_mask(id, None),
        ComponentKind::Text => world.set_text(id, None),
        ComponentKind::Selectable => world.set_selectable(id, None),
        ComponentKind::LayoutGroup => world.set_layout_group(id, None),
        ComponentKind::LayoutElement => world.set_layout_element(id, None),
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
    for &dependent in ComponentKind::ALL {
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
        for &kind in ComponentKind::ALL {
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
    for &kind in ComponentKind::ALL {
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
#[path = "dependency_tests.rs"]
mod tests;

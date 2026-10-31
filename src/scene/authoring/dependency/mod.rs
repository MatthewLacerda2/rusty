//! src/scene/authoring/dependency/mod.rs — Component dependency mechanism (#359).
//!
//! rusty's `RequireComponent` (Unity's `[RequireComponent(typeof(T))]`), promoted
//! from the hand-rolled Camera↔PostProcessing one-off into a declared rule enforced
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
//! asset; it has no requirements and is required by nothing, so its add never flows
//! through here — `set_default`'s Material arm exists only for completeness and stages
//! the default asset the same way the editor Add menu does. Removing a Mesh also
//! drops the Material reference ([`remove_with_cascade`], #1657): in Unity the
//! materials live on the renderer.

mod dispatch;

pub use dispatch::has_kind;
pub(crate) use dispatch::{clear_one, set_default};

use crate::ecs::World;
use crate::scene::authoring::components::ComponentKind;

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
/// shipped Camera→PostProcessing cascade, now driven by [`ComponentKind::requires`].
/// Returns `false` when the entity is missing.
pub fn remove_with_cascade(world: &mut World, id: u32, kind: ComponentKind) -> bool {
    for &dependent in ComponentKind::ALL {
        if dependent.requires().contains(&kind) && has_kind(world, id, dependent) {
            remove_with_cascade(world, id, dependent);
        }
    }
    // rusty's Mesh is Unity's Mesh Filter and Mesh Renderer in one, and Unity keeps
    // a renderer's materials on the renderer: removing the Mesh drops the entity's
    // material reference too (#1657). Not a `requires`, which would also make adding
    // a Material add a Mesh. The library asset stays, as the Material card's remove
    // leaves it.
    if kind == ComponentKind::Mesh {
        clear_one(world, id, ComponentKind::Texture);
    }
    clear_one(world, id, kind)
}

/// Drop any component on `id` whose declared requirements are unmet, repeating until
/// stable (removing a dependent can orphan its own dependents). The editor's per-frame
/// safety net — the reactive form of the remove cascade, catching drift the add/remove
/// verbs didn't produce (a Camera cleared out from under a PostProcessing by another
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
/// added so the caller can warn — hand-edited scenes keep loading instead of
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
mod tests;

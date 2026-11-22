mod dispatch;

pub use dispatch::has_kind;
pub(crate) use dispatch::{clear_one, set_default};

use crate::authoring::components::ComponentKind;
use rusty_components::{ColliderShape, SharedMesh};
use rusty_ecs::World;

pub fn satisfy_requirements(world: &mut World, id: u32, kind: ComponentKind) {
    for &req in kind.requires() {
        if !has_kind(world, id, req) {
            set_default(world, id, req);
            satisfy_requirements(world, id, req);
        }
    }
}

pub fn add_with_requirements(world: &mut World, id: u32, kind: ComponentKind) -> bool {
    if !set_default(world, id, kind) {
        return false;
    }
    satisfy_requirements(world, id, kind);
    true
}

pub fn remove_with_cascade(world: &mut World, id: u32, kind: ComponentKind) -> bool {
    for &dependent in ComponentKind::ALL {
        if dependent.requires().contains(&kind) && has_kind(world, id, dependent) {
            remove_with_cascade(world, id, dependent);
        }
    }
    // The Mesh is Unity's Mesh Filter and Renderer in one, so it takes the Material (`Texture`)
    // along; not a `requires`, which would make adding a Material add a Mesh.
    if kind == ComponentKind::Mesh {
        clear_one(world, id, ComponentKind::Texture);
        keep_collider_mesh(world, id);
    }
    clear_one(world, id, kind)
}

/// Before the Mesh clears: a Mesh-shaped collider keeps it as `shared_mesh`, so the entity keeps
/// colliding.
fn keep_collider_mesh(world: &mut World, id: u32) {
    let Some(shared) = world.mesh(id).map(|m| SharedMesh::of(&m)) else {
        return;
    };
    if let Some(mut collider) = world.collider_mut(id) {
        if matches!(collider.shape, ColliderShape::Mesh { .. }) {
            collider.shared_mesh = Some(shared);
        }
    }
}

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

//! `Mesh.DeleteAsset` (#2059): take a mesh out of the scene's library, as deleting a
//! mesh asset in Unity does, by the rules `Material.DeleteAsset` keeps (#1576).
//!
//! A mesh still in use is refused, every user named. With `force` each user lets go
//! of it: an entity drawing it loses its Mesh (Unity's `MeshFilter` left with no
//! mesh draws nothing), and a Mesh collider keeping a copy of it (#1657) drops the
//! copy, so it builds from nothing. The console warns which. The renderer frees the
//! mesh's GPU buffers on its next frame (`render::scene::extract`).

use super::{kept_mesh_name, scene_mesh_name, scene_mesh_users};
use crate::scene::Scene;

impl Scene {
    /// Delete library mesh `name`; returns the users `force` moved off it, by id
    /// (none when it was unused). See the module docs.
    pub fn delete_mesh(&mut self, name: &str, force: bool) -> Result<Vec<u32>, String> {
        if !self.meshes.contains_key(name) {
            return Err(format!("no mesh named {name:?}"));
        }
        let users = self.mesh_users(name);
        let named = users
            .iter()
            .map(|&id| self.describe_user(id))
            .collect::<Vec<_>>()
            .join("; ");
        if !users.is_empty() && !force {
            return Err(format!(
                "mesh {name:?} is used by {named}; pass {{ force = true }} to take it off them"
            ));
        }
        self.meshes.remove(name);
        for &id in &users {
            let own = self
                .world
                .mesh(id)
                .is_some_and(|m| scene_mesh_name(&m) == Some(name));
            if own {
                self.world.set_mesh(id, None);
            }
            let kept = self
                .world
                .collider(id)
                .is_some_and(|c| kept_mesh_name(&c) == Some(name));
            if kept {
                if let Some(mut collider) = self.world.collider_mut(id) {
                    collider.shared_mesh = None;
                }
                self.world.refresh_collider_aabb(id);
            }
        }
        if !users.is_empty() {
            log::warn!("[Mesh] {name:?} deleted; it no longer draws on {named}");
        }
        Ok(users)
    }

    /// The entities using library mesh `name` (`scene_mesh_users`), ascending.
    pub fn mesh_users(&self, name: &str) -> Vec<u32> {
        let mut ids = self.world.ids().to_vec();
        ids.retain(|&id| {
            let (mesh, collider) = (self.world.mesh(id), self.world.collider(id));
            let used = scene_mesh_users(mesh.as_deref(), collider.as_deref()).any(|n| n == name);
            used
        });
        ids.sort_unstable();
        ids
    }

    /// `entity 3 "Plank"`, with `(instance of <prefab>)` for a prefab instance's.
    fn describe_user(&self, id: u32) -> String {
        let name = self.world.name(id).map(|n| n.clone()).unwrap_or_default();
        let link = self.world.prefab_link(id).map(|l| l.source.clone());
        let prefab = link
            .map(|p| format!(" (instance of {p})"))
            .unwrap_or_default();
        format!("entity {id} {name:?}{prefab}")
    }
}

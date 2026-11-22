use glam::Vec3;

use super::{kept_mesh_name, scene_mesh_name, scene_mesh_users};
use crate::Scene;
use rusty_components::ColliderShape;

impl Scene {
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
            }
            self.shrink_emptied_collider(id);
        }
        if !users.is_empty() {
            log::warn!("[Mesh] {name:?} deleted; it no longer draws on {named}");
        }
        Ok(users)
    }

    fn shrink_emptied_collider(&mut self, id: u32) {
        let own = self.world.mesh(id).is_some();
        if let Some(mut collider) = self.world.collider_mut(id) {
            let empty =
                !own && collider.shared_mesh.is_none() && collider.built_collision_mesh().is_none();
            if let ColliderShape::Mesh {
                local_min,
                local_max,
                ..
            } = &mut collider.shape
            {
                if empty {
                    (*local_min, *local_max) = (Vec3::ZERO, Vec3::ZERO);
                }
            }
        }
        self.world.refresh_collider_aabb(id);
    }

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

    fn describe_user(&self, id: u32) -> String {
        let name = self.world.name(id).map(|n| n.clone()).unwrap_or_default();
        let link = self.world.prefab_link(id).map(|l| l.source.clone());
        let prefab = link
            .map(|p| format!(" (instance of {p})"))
            .unwrap_or_default();
        format!("entity {id} {name:?}{prefab}")
    }
}

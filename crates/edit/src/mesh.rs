use crate::authoring::ComponentKind;
use rusty_scene::mesh_library::{put_shared, resolve_shared};
use rusty_scene::Scene;

pub fn set_shared_mesh(scene: &mut Scene, id: u32, reference: &str) -> Result<(), String> {
    if !scene.world.contains(id) {
        return Err(format!("no entity {id}"));
    }
    let fresh = resolve_shared(scene, reference)?;
    if scene.world.mesh(id).is_none() {
        crate::authoring::add_component(scene, id, ComponentKind::Mesh);
    }
    put_shared(scene, id, fresh);
    Ok(())
}

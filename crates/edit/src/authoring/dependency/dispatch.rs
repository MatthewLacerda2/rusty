use crate::authoring::components::ComponentKind;
use crate::authoring::defaults::{
    default_animator, default_camera, default_collider, default_light, default_material,
    default_nav_agent, default_rigidbody,
};
use crate::authoring::{primitive_mesh_component, Primitive};
use rusty_components::registry::first_class;
use rusty_ecs::World;
use rusty_scene::rehydrate::{bounds_of, refit_collider};
use rusty_scene::MaterialComponent;

macro_rules! added {
    ($w:ident, $id:ident, mesh, $set:ident) => {
        add_box_mesh($w, $id)
    };
    ($w:ident, $id:ident, material, $set:ident) => {
        attach_default_material_world($w, $id)
    };
    ($w:ident, $id:ident, animator, $set:ident) => {
        $w.$set($id, Some(default_animator()))
    };
    ($w:ident, $id:ident, camera, $set:ident) => {
        $w.$set($id, Some(default_camera()))
    };
    ($w:ident, $id:ident, collider, $set:ident) => {
        $w.$set($id, Some(default_collider()))
    };
    ($w:ident, $id:ident, light, $set:ident) => {
        $w.$set($id, Some(default_light()))
    };
    ($w:ident, $id:ident, nav_agent, $set:ident) => {
        $w.$set($id, Some(default_nav_agent()))
    };
    ($w:ident, $id:ident, rigidbody, $set:ident) => {
        $w.$set($id, Some(default_rigidbody()))
    };
    ($w:ident, $id:ident, $field:ident, $set:ident) => {
        $w.$set($id, Some(Default::default()))
    };
}

macro_rules! dispatch {
    ($($field:ident: $ty:ty => $kind:ident [$get_mut:ident $has:ident $set:ident $take:ident $ids:ident];)*) => {
        pub fn has_kind(world: &World, id: u32, kind: ComponentKind) -> bool {
            match kind {
                $(ComponentKind::$kind => world.$has(id),)*
            }
        }

        pub(crate) fn set_default(world: &mut World, id: u32, kind: ComponentKind) -> bool {
            match kind {
                $(ComponentKind::$kind => added!(world, id, $field, $set),)*
            }
        }

        pub(crate) fn clear_one(world: &mut World, id: u32, kind: ComponentKind) -> bool {
            match kind {
                $(ComponentKind::$kind => world.$set(id, None),)*
            }
        }
    };
}

first_class!(dispatch);

fn add_box_mesh(world: &mut World, id: u32) -> bool {
    let mesh = primitive_mesh_component(Primitive::Box);
    let bounds = bounds_of(mesh.iter().flat_map(|m| &m.vertices).map(|v| v.position));
    if !world.set_mesh(id, mesh) {
        return false;
    }
    if let Some(mut collider) = world.collider_mut(id) {
        collider.shared_mesh = None;
        refit_collider(&mut collider, bounds);
    }
    true
}

fn attach_default_material_world(world: &mut World, id: u32) -> bool {
    if !world.set_material(id, Some(MaterialComponent::for_entity(id))) {
        return false;
    }
    world.stage_pending_material(id, default_material());
    true
}

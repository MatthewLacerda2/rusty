//! #827: the three layer settings an agent can now set from Lua — a slot's name,
//! the collision matrix and a camera's culling mask. `values` checks each call
//! writes the scene and survives a save/load; `contact` checks a matrix change made
//! mid-play actually stops a contact.

mod contact;
mod values;

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::api::bind;
use rusty::scene::Scene;

/// Run `f` with `Layers`, `Physics` and `Camera` bound over `scene`.
pub fn with_api(scene: &Rc<RefCell<Scene>>, f: impl FnOnce(&Lua)) {
    let lua = Lua::new();
    crate::lua::attempt(|| {
        rusty::api::layers::register(&lua, bind(&lua, scene)).unwrap();
        rusty::api::physics::register(&lua, bind(&lua, scene)).unwrap();
        let camera = lua.create_table()?;
        rusty::api::camera::component::register(&lua, &camera, bind(&lua, scene)).unwrap();
        lua.globals().set("Camera", camera)?;
        f(&lua);
        Ok(())
    })
    .unwrap();
}

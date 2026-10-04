//! src/api/physics/hitbox.rs — `Physics.GenerateHitboxes` (#464).
//!
//! Fits one hitbox collider per bone of a skinned character, the same
//! `Scene::generate_hitboxes` the inspector's *Generate Hitboxes* button runs.
//! Unity has no runtime equivalent (its Ragdoll Wizard is editor-only); here the
//! agent can do anything the editor can.

use std::cell::RefCell;

use mlua::Table;

use super::super::{put, Reg};
use crate::scene::skeleton::HitboxOptions;
use crate::scene::Scene;

/// Register `GenerateHitboxes` onto the `Physics` table.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GenerateHitboxes",
        scope.create_function(|lua, (id, opts): (u32, Option<Table>)| {
            let opts = options(opts)?;
            let made = scene
                .borrow_mut()
                .generate_hitboxes(id, &opts)
                .map_err(mlua::Error::RuntimeError)?;
            let out = lua.create_table()?;
            for (bone, hitbox) in made {
                out.set(bone, hitbox)?;
            }
            Ok(out)
        }),
    )
}

/// Read the optional `{ bones = {..}, min_size = n, layer = "name" }` table over
/// the defaults.
fn options(t: Option<Table>) -> mlua::Result<HitboxOptions> {
    let mut opts = HitboxOptions::default();
    let Some(t) = t else {
        return Ok(opts);
    };
    if let Some(bones) = t.get::<Option<Vec<String>>>("bones")? {
        opts.bones = Some(bones);
    }
    if let Some(min_size) = t.get::<Option<f32>>("min_size")? {
        opts.min_size = min_size;
    }
    if let Some(layer) = t.get::<Option<String>>("layer")? {
        opts.layer = layer;
    }
    Ok(opts)
}

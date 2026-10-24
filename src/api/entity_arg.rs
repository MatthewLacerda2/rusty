//! src/api/entity_arg.rs — the one way a binding takes an entity id from Lua (#1383).
//!
//! A script passes an id it got from the API, and the API answers `nil` for "no
//! entity" (`Scene.FindEntityByName` on a missing name, `Scene.FindChild` on a
//! missing path). Every id-taking binding takes an [`EntityArg`], never a bare
//! `u32`, so that `nil` means the same thing everywhere (`make rules`'
//! `entity-arg` holds it):
//!
//!   - a **read** ([`EntityArg::read`]) treats `nil` as a missing entity and
//!     answers its missing-entity default, as it does for a dead id;
//!   - a **write** ([`EntityArg::live`]) follows Unity, where calling a member on a
//!     destroyed object throws: `nil` is a Lua error naming the verb
//!     (`"Scene.SetActive: no entity (nil id)"`). A dead id stays the write's
//!     no-op, as before.
//!   - where `nil` means "none" on purpose (a joint's connected body, a selectable's
//!     explicit target), [`EntityArg::get`] gives the `Option`.

use mlua::{FromLua, Lua, Value};

use crate::ecs::NO_ENTITY;

/// An entity id as Lua passes it: an integer, or `nil` for no entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityArg(Option<u32>);

impl EntityArg {
    /// The id for a read: `nil` reads as [`NO_ENTITY`], which names no entity, so
    /// the read answers what it answers for a missing one.
    pub fn read(self) -> u32 {
        self.0.unwrap_or(NO_ENTITY)
    }

    /// The id for a write `namespace.verb`, or a Lua error naming it for `nil`.
    pub fn live(self, namespace: &str, verb: &str) -> mlua::Result<u32> {
        self.0.ok_or_else(|| {
            mlua::Error::RuntimeError(format!("{namespace}.{verb}: no entity (nil id)"))
        })
    }

    /// The id, or `None` for `nil`: for an argument where `nil` means "none".
    pub fn get(self) -> Option<u32> {
        self.0
    }
}

impl From<u32> for EntityArg {
    fn from(id: u32) -> Self {
        Self(Some(id))
    }
}

impl FromLua for EntityArg {
    fn from_lua(value: Value, lua: &Lua) -> mlua::Result<Self> {
        match value {
            Value::Nil => Ok(Self(None)),
            other => u32::from_lua(other, lua).map(|id| Self(Some(id))),
        }
    }
}

#[cfg(test)]
#[path = "entity_arg_tests.rs"]
mod tests;

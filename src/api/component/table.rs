//! src/api/component/table.rs — the field table behind `Component.*` (#1369).
//!
//! Each first-class component lists its serialized fields once, as [`Field`]s: a
//! field is either written through the `scene::authoring` op the inspector's card
//! calls ([`Access::Op`]) or read-only with the reason ([`Access::ReadOnly`]). There
//! is no serde write-back: a field with no op cannot be written, so the inspector's
//! clamping, sanitizing and side effects are the only way in. `tests.rs` fails when a
//! component's serialized fields and its table disagree.
//!
//! [`Kind`] erases the component type so the namespace can pick one by name.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::ecs::World;
use crate::scene::authoring::ComponentKind;

/// Writes one field of `C` from its serialized value, through the field's op.
pub type Write<C> = fn(&mut C, Value) -> Result<(), String>;

/// How a field may be reached from `Component.Set`.
pub enum Access<C> {
    /// Written by the inspector's op (decode the value, then call it).
    Op(Write<C>),
    /// Readable only; the text says why, or which verb writes it.
    ReadOnly(&'static str),
}

/// One serialized field of a component, by its serialized name.
pub struct Field<C> {
    pub name: &'static str,
    pub access: Access<C>,
}

/// Decode a field's value as `T` (enum names in any case, `lua_json::decode`). An
/// empty Lua table crosses as `{}`, so an empty list read back by `Get` decodes as
/// `[]` when `{}` does not.
pub fn decode<T: DeserializeOwned>(json: Value) -> Result<T, String> {
    let empty = json.as_object().is_some_and(Map::is_empty);
    crate::api::lua_json::decode(json).or_else(|e| {
        if empty {
            crate::api::lua_json::decode(Value::Array(Vec::new())).map_err(|_| e)
        } else {
            Err(e)
        }
    })
}

/// A field written by one op, `fn(&mut C, T)` with `T` decoded from the value.
macro_rules! op {
    ($name:literal, $op:path) => {
        $crate::api::component::table::Field {
            name: $name,
            access: $crate::api::component::table::Access::Op(|c, json| {
                $op(c, $crate::api::component::table::decode(json)?);
                Ok(())
            }),
        }
    };
}

/// A field written by an adapter over its op(s): the value decoded as `$t`, then
/// `$body` (which may use `?` on a fallible op).
macro_rules! with {
    ($name:literal, |$c:ident, $v:ident: $t:ty| $body:expr) => {
        $crate::api::component::table::Field {
            name: $name,
            access: $crate::api::component::table::Access::Op(|$c, json| {
                let $v: $t = $crate::api::component::table::decode(json)?;
                $body;
                Ok(())
            }),
        }
    };
}

/// A read-only field and why it is read-only.
macro_rules! ro {
    ($name:literal, $why:expr) => {
        $crate::api::component::table::Field {
            name: $name,
            access: $crate::api::component::table::Access::ReadOnly($why),
        }
    };
}

pub(super) use {op, ro, with};

/// A first-class component the table covers: its fields, and how to reach it on
/// an entity.
pub trait Inspect: Clone + Serialize + DeserializeOwned + 'static {
    const FIELDS: &'static [Field<Self>];
    /// The entity's component, serialized, or `None` without one.
    fn read(world: &World, id: u32) -> Option<Result<Value, String>>;
    /// Run `f` on the entity's component; `false` without one.
    fn update(world: &mut World, id: u32, f: &mut dyn FnMut(&mut Self)) -> bool;
}

/// Implement [`Inspect`] for each `Type => getter, mutable getter, field table`.
macro_rules! inspect {
    ($($ty:ty => $get:ident, $get_mut:ident, $fields:expr;)*) => {$(
        impl $crate::api::component::table::Inspect for $ty {
            const FIELDS: &'static [$crate::api::component::table::Field<Self>] = $fields;
            fn read(
                world: &$crate::ecs::World,
                id: u32,
            ) -> Option<Result<serde_json::Value, String>> {
                world
                    .$get(id)
                    .map(|c| serde_json::to_value(&*c).map_err(|e| e.to_string()))
            }
            fn update(
                world: &mut $crate::ecs::World,
                id: u32,
                f: &mut dyn FnMut(&mut Self),
            ) -> bool {
                world.$get_mut(id).map(|mut c| f(&mut c)).is_some()
            }
        }
    )*};
}

pub(super) use inspect;

/// One field as `Component.Fields` lists it: its name, and why it is read-only
/// (`None`: writable).
pub type FieldInfo = (&'static str, Option<&'static str>);

/// Applies a change table to an entity's component; `Ok(false)` without one.
pub type WriteAny = fn(&mut World, u32, &Map<String, Value>) -> Result<bool, String>;

/// A component the namespace can name, with its type erased.
pub struct Kind {
    /// The name `Component.*` answers to first (any name `Scene.AddComponent`
    /// takes resolves too, through `add`).
    pub name: &'static str,
    /// The Add Component kind, `None` for `Transform` and `Mesh`, which the menu
    /// does not add.
    pub add: Option<ComponentKind>,
    /// The component's key in a saved entity (`Entity`'s field name; the table test).
    #[cfg(test)]
    pub key: &'static str,
    pub read: fn(&World, u32) -> Option<Result<Value, String>>,
    pub write: WriteAny,
    pub fields: fn() -> Vec<FieldInfo>,
    /// The field names the component's serde derive declares (the table test).
    #[cfg(test)]
    pub serde_fields: fn() -> Option<&'static [&'static str]>,
}

/// The [`Kind`] for `C`.
pub const fn kind<C: Inspect>(
    name: &'static str,
    add: Option<ComponentKind>,
    key: &'static str,
) -> Kind {
    #[cfg(not(test))]
    let _ = key;
    Kind {
        name,
        add,
        #[cfg(test)]
        key,
        read: C::read,
        write: write::<C>,
        fields: fields::<C>,
        #[cfg(test)]
        serde_fields: super::introspect::serde_fields::<C>,
    }
}

fn fields<C: Inspect>() -> Vec<FieldInfo> {
    C::FIELDS
        .iter()
        .map(|f| match f.access {
            Access::Op(_) => (f.name, None),
            Access::ReadOnly(why) => (f.name, Some(why)),
        })
        .collect()
}

/// Write `changes` onto `id`'s `C`, all or nothing: every key is checked before
/// anything is written, and the ops run on a copy that replaces the component only
/// when every one succeeded. They run in the table's order, not the caller's, so
/// a field whose op resets another (a reverb `preset` its `params`) is listed after
/// it and wins.
fn write<C: Inspect>(
    world: &mut World,
    id: u32,
    changes: &Map<String, Value>,
) -> Result<bool, String> {
    for key in changes.keys() {
        let field = C::FIELDS.iter().find(|f| f.name == key).ok_or_else(|| {
            let names: Vec<&str> = C::FIELDS.iter().map(|f| f.name).collect();
            format!("unknown field '{key}' (fields: {})", names.join(", "))
        })?;
        if let Access::ReadOnly(why) = field.access {
            return Err(format!("field '{key}' is read-only: {why}"));
        }
    }
    let mut ops: Vec<_> = C::FIELDS
        .iter()
        .filter_map(|f| match (&f.access, changes.get(f.name)) {
            (Access::Op(op), Some(value)) => Some((f.name, *op, value.clone())),
            _ => None,
        })
        .collect();
    let mut outcome = Ok(());
    let found = C::update(world, id, &mut |live| {
        let mut draft = live.clone();
        outcome = ops.drain(..).try_for_each(|(name, op, value)| {
            op(&mut draft, value).map_err(|e| format!("{name}: {e}"))
        });
        if outcome.is_ok() {
            *live = draft;
        }
    });
    outcome.map(|()| found)
}

//! src/scene/authoring/fields/table.rs — the field table behind `Component.*` (#1369)
//! and the load-time hold (#1418).
//!
//! Each first-class component lists its serialized fields once, as [`Field`]s: a
//! field is either written through the `scene::authoring` op the inspector's card
//! calls ([`Access::Op`]) or read-only with the reason ([`Access::ReadOnly`]). There
//! is no serde write-back: a field with no op cannot be written, so the inspector's
//! clamping, sanitizing and side effects are the only way in. Fields one op writes
//! together (a light's two cone angles) form a [`Group`], written once per `Set`. `tests.rs` fails when a
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
pub enum Access<C: 'static> {
    /// Written by the inspector's op (decode the value, then call it).
    Op(Write<C>),
    /// Written with the other fields of its group, by one op (`group!`).
    Group(Group<C>),
    /// An object written key by key (a post effect's settings, `effect!`): `Set`
    /// skips its unchanged leaves, not the whole object (`value::changed`).
    Merge(Write<C>),
    /// Readable only; the text says why, or which verb writes it.
    ReadOnly(&'static str),
}

/// Fields one op writes together, `set_cones(light, inner, outer)`: a `Set`
/// naming any of them calls it once, with each field's new value or, when not
/// named, its current one. So an op that clamps one half against the other sees
/// both halves as they are meant to end up.
pub struct Group<C: 'static> {
    pub names: &'static [&'static str],
    pub write: fn(&mut C, &Values) -> Result<(), String>,
}

// Copy by hand: a derive would ask `C: Copy` of the component, which only the fn
// pointers and static slices inside need not be.
impl<C> Clone for Group<C> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<C> Copy for Group<C> {}
impl<C> Clone for Access<C> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<C> Copy for Access<C> {}
impl<C> Clone for Field<C> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<C> Copy for Field<C> {}

/// What a group's op reads its fields from: the changes, else the component.
pub struct Values<'a> {
    pub changes: &'a Map<String, Value>,
    pub current: &'a Value,
}

impl Values<'_> {
    /// Field `name`'s value as `T`: the one written, else the one held.
    pub fn take<T: DeserializeOwned>(&self, name: &str) -> Result<T, String> {
        let value = self.changes.get(name).or_else(|| self.current.get(name));
        super::value::decode(value.cloned().unwrap_or(Value::Null))
            .map_err(|e| format!("{name}: {e}"))
    }
}

/// One serialized field of a component, by its serialized name. `Copy`, so a table
/// can be assembled in a `const` from rows other modules declare (the post effects',
/// `rendering::post_processing::FIELDS`).
pub struct Field<C: 'static> {
    pub name: &'static str,
    pub access: Access<C>,
    /// Why a scene or prefab load leaves this field as read instead of running its
    /// op (`unheld!`); `None`: a load holds it (#1418).
    pub unheld: Option<&'static str>,
}

/// A field written by one op, `fn(&mut C, T)` with `T` decoded from the value.
macro_rules! op {
    ($name:literal, $op:path) => {
        $crate::scene::authoring::fields::table::Field {
            name: $name,
            access: $crate::scene::authoring::fields::table::Access::Op(|c, json| {
                $op(c, $crate::scene::authoring::fields::value::decode(json)?);
                Ok(())
            }),
            unheld: None,
        }
    };
}

/// A field written by an adapter over its op(s): the value decoded as `$t`, then
/// `$body` (which may use `?` on a fallible op).
macro_rules! with {
    ($name:literal, |$c:ident, $v:ident: $t:ty| $body:expr) => {
        $crate::scene::authoring::fields::table::Field {
            name: $name,
            access: $crate::scene::authoring::fields::table::Access::Op(|$c, json| {
                let $v: $t = $crate::scene::authoring::fields::value::decode(json)?;
                $body;
                Ok(())
            }),
            unheld: None,
        }
    };
}

/// A [`Group`]: `|component, field: Type, …| body`, each field named as its
/// binding. List each of its fields with `part!`.
macro_rules! group {
    (|$c:ident $(, $f:ident: $t:ty)+ $(,)?| $body:expr) => {
        $crate::scene::authoring::fields::table::Group {
            names: &[$(stringify!($f)),+],
            write: |$c, values| {
                $(let $f: $t = values.take(stringify!($f))?;)+
                $body;
                Ok(())
            },
        }
    };
}

/// A field written by its [`Group`]'s op.
macro_rules! part {
    ($name:literal, $group:expr) => {
        $crate::scene::authoring::fields::table::Field {
            name: $name,
            access: $crate::scene::authoring::fields::table::Access::Group($group),
            unheld: None,
        }
    };
}

/// A post effect's settings object (#1467): `$name` is both the serialized field and
/// the component's field, read as `$t` **over the current value** (`value::merged`),
/// so `{ bloom = { intensity = 2 } }` keeps the rest and a misspelt key is an error.
/// Then `$body`, which writes each part through its op.
macro_rules! effect {
    ($name:ident, |$c:ident, $v:ident: $t:ty| $body:expr) => {
        $crate::scene::authoring::fields::table::Field {
            name: stringify!($name),
            access: $crate::scene::authoring::fields::table::Access::Merge(|$c, json| {
                let $v: $t = $crate::scene::authoring::fields::value::merged(&$c.$name, json)?;
                $body;
                Ok(())
            }),
            unheld: None,
        }
    };
}

/// A read-only field and why it is read-only.
macro_rules! ro {
    ($name:literal, $why:expr) => {
        $crate::scene::authoring::fields::table::Field {
            name: $name,
            access: $crate::scene::authoring::fields::table::Access::ReadOnly($why),
            unheld: None,
        }
    };
}

/// `$field` (any field above) left as read by a load, and why: its op reads from
/// disk or warns, so running it on every load costs more than the bound it holds.
macro_rules! unheld {
    ($why:literal, $field:expr) => {
        $crate::scene::authoring::fields::table::Field {
            unheld: Some($why),
            ..$field
        }
    };
}

pub(in crate::scene::authoring) use {effect, group, op, part, ro, unheld, with};

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
        impl $crate::scene::authoring::fields::table::Inspect for $ty {
            const FIELDS: &'static [$crate::scene::authoring::fields::table::Field<Self>] = $fields;
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
pub type WriteAny =
    fn(&mut World, u32, &Map<String, Value>, super::write::Skip) -> Result<bool, String>;

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
    /// Hold a loaded component's numbers through their ops (#1418); the last
    /// argument is the component's name, for the warnings.
    pub hold: fn(&mut World, u32, &'static str),
    pub fields: fn() -> Vec<FieldInfo>,
    /// A field's type as its derive reads it: optional, and the Lua type it holds.
    pub field_type: fn(&'static str) -> (bool, Option<&'static str>),
    /// The field names the component's serde derive declares (the table test).
    #[cfg(test)]
    pub serde_fields: fn() -> Option<&'static [&'static str]>,
    /// Whether a load runs field `name`'s op (the hold tests).
    #[cfg(test)]
    pub held_on_load: fn(&str) -> bool,
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
        write: super::write::write::<C>,
        hold: super::hold::hold::<C>,
        fields: fields::<C>,
        field_type: super::introspect::field_type::<C>,
        #[cfg(test)]
        serde_fields: super::introspect::serde_fields::<C>,
        #[cfg(test)]
        held_on_load: super::hold::named_held::<C>,
    }
}

fn fields<C: Inspect>() -> Vec<FieldInfo> {
    C::FIELDS
        .iter()
        .map(|f| match &f.access {
            Access::ReadOnly(why) => (f.name, Some(*why)),
            Access::Group(g) => {
                // Checked wherever the table is listed, the table tests included.
                debug_assert!(g.names.contains(&f.name), "{} not in its group", f.name);
                (f.name, None)
            }
            Access::Op(_) | Access::Merge(_) => (f.name, None),
        })
        .collect()
}

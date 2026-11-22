use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::authoring::ComponentKind;
use rusty_ecs::World;

pub type Write<C> = fn(&mut C, Value) -> Result<(), String>;

/// No serde write-back: a field without an op cannot be written, so the inspector's clamps and side
/// effects are the only way in.
pub enum Access<C: 'static> {
    Op(Write<C>),
    Group(Group<C>),
    Merge(Write<C>),
    ReadOnly(&'static str),
}

/// A `Set` naming any part calls the op once, each part not named at its current value, so a clamp
/// of one half against the other sees both as they will end up.
pub struct Group<C: 'static> {
    pub names: &'static [&'static str],
    pub write: fn(&mut C, &Values) -> Result<(), String>,
}

// `Clone` and `Copy` by hand: a derive would ask `C: Copy` of the component.
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

pub struct Values<'a> {
    pub changes: &'a Map<String, Value>,
    pub current: &'a Value,
}

impl Values<'_> {
    pub fn take<T: DeserializeOwned>(&self, name: &str) -> Result<T, String> {
        let value = self.changes.get(name).or_else(|| self.current.get(name));
        super::value::decode(value.cloned().unwrap_or(Value::Null))
            .map_err(|e| format!("{name}: {e}"))
    }
}

pub struct Field<C: 'static> {
    pub name: &'static str,
    pub access: Access<C>,
    pub unheld: Option<&'static str>,
}

macro_rules! op {
    ($name:literal, $op:path) => {
        $crate::authoring::fields::table::Field {
            name: $name,
            access: $crate::authoring::fields::table::Access::Op(|c, json| {
                $op(c, $crate::authoring::fields::value::decode(json)?);
                Ok(())
            }),
            unheld: None,
        }
    };
}

macro_rules! with {
    ($name:literal, |$c:ident, $v:ident: $t:ty| $body:expr) => {
        $crate::authoring::fields::table::Field {
            name: $name,
            access: $crate::authoring::fields::table::Access::Op(|$c, json| {
                let $v: $t = $crate::authoring::fields::value::decode(json)?;
                $body;
                Ok(())
            }),
            unheld: None,
        }
    };
}

macro_rules! group {
    (|$c:ident $(, $f:ident: $t:ty)+ $(,)?| $body:expr) => {
        $crate::authoring::fields::table::Group {
            names: &[$(stringify!($f)),+],
            write: |$c, values| {
                $(let $f: $t = values.take(stringify!($f))?;)+
                $body;
                Ok(())
            },
        }
    };
}

macro_rules! part {
    ($name:literal, $group:expr) => {
        $crate::authoring::fields::table::Field {
            name: $name,
            access: $crate::authoring::fields::table::Access::Group($group),
            unheld: None,
        }
    };
}

macro_rules! effect {
    ($name:ident, |$c:ident, $v:ident: $t:ty| $body:expr) => {
        $crate::authoring::fields::table::Field {
            name: stringify!($name),
            access: $crate::authoring::fields::table::Access::Merge(|$c, json| {
                let $v: $t = $crate::authoring::fields::value::merged(&$c.$name, json)?;
                $body;
                Ok(())
            }),
            unheld: None,
        }
    };
}

macro_rules! ro {
    ($name:literal, $why:expr) => {
        $crate::authoring::fields::table::Field {
            name: $name,
            access: $crate::authoring::fields::table::Access::ReadOnly($why),
            unheld: None,
        }
    };
}

macro_rules! unheld {
    ($why:literal, $field:expr) => {
        $crate::authoring::fields::table::Field {
            unheld: Some($why),
            ..$field
        }
    };
}

pub(in crate::authoring) use {effect, group, op, part, ro, unheld, with};

pub(super) use super::inspect::{inspect, Inspect};

pub type FieldInfo = (&'static str, Option<&'static str>);

pub type WriteAny =
    fn(&mut World, u32, &Map<String, Value>, super::write::Skip) -> Result<bool, String>;

pub struct Kind {
    pub name: &'static str,
    pub add: Option<ComponentKind>,
    #[cfg(test)]
    pub key: &'static str,
    pub read: fn(&World, u32) -> Option<Result<Value, String>>,
    pub write: WriteAny,
    /// The last argument is the component's name, for the load warnings.
    pub hold: fn(&mut World, u32, &'static str),
    pub fields: fn() -> Vec<FieldInfo>,
    pub field_type: fn(&'static str) -> (bool, Option<&'static str>),
    #[cfg(test)]
    pub serde_fields: fn() -> Option<&'static [&'static str]>,
    #[cfg(test)]
    pub held_on_load: fn(&str) -> bool,
}

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
        field_type: C::field_type,
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
                debug_assert!(g.names.contains(&f.name), "{} not in its group", f.name);
                (f.name, None)
            }
            Access::Op(_) | Access::Merge(_) => (f.name, None),
        })
        .collect()
}

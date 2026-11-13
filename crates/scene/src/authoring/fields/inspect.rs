//! crates/scene/src/authoring/fields/inspect.rs — how the field table reaches a component.
//!
//! [`Inspect`] is what `table::kind` erases: a component's field table and how to
//! read and update it on an entity. [`inspect!`] implements it from a getter pair.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use super::table::Field;
use rusty_ecs::World;

/// A first-class component the table covers: its fields, and how to reach it on
/// an entity.
pub trait Inspect: Clone + Serialize + DeserializeOwned + 'static {
    const FIELDS: &'static [Field<Self>];
    /// The entity's component, serialized, or `None` without one.
    fn read(world: &World, id: u32) -> Option<Result<Value, String>>;
    /// Run `f` on the entity's component; `false` without one.
    fn update(world: &mut World, id: u32, f: &mut dyn FnMut(&mut Self)) -> bool;
    /// A field's type as its derive reads it: optional, and the Lua type it holds.
    fn field_type(name: &'static str) -> (bool, Option<&'static str>) {
        super::introspect::field_type::<Self>(name)
    }
}

/// Implement [`Inspect`] for each `Type => getter, mutable getter, field table`,
/// optionally followed by `shown <fn(&Type, &mut Value)>`: what `read` reports
/// where it differs from the stored value (a Light's `effective_mode`, #1674), and
/// by `field_type <fn(&'static str) -> (bool, Option<&'static str>)>`: where a
/// field's type is read when the derive alone does not say (an Animator's flattened
/// `Playback`).
macro_rules! inspect {
    ($($ty:ty => $get:ident, $get_mut:ident, $fields:expr $(, shown $shown:expr)? $(, field_type $ft:expr)?;)*) => {$(
        impl $crate::authoring::fields::table::Inspect for $ty {
            const FIELDS: &'static [$crate::authoring::fields::table::Field<Self>] = $fields;
            fn read(world: &::rusty_ecs::World, id: u32) -> Option<Result<serde_json::Value, String>> {
                world.$get(id).map(|c| {
                    let value = serde_json::to_value(&*c).map_err(|e| e.to_string())?;
                    $( let mut value = value; ($shown)(&*c, &mut value); )?
                    Ok(value)
                })
            }
            fn update(world: &mut ::rusty_ecs::World, id: u32, f: &mut dyn FnMut(&mut Self)) -> bool {
                world.$get_mut(id).map(|mut c| f(&mut c)).is_some()
            }
            $(
                fn field_type(name: &'static str) -> (bool, Option<&'static str>) {
                    ($ft)(name)
                }
            )?
        }
    )*};
}

pub(in crate::authoring::fields) use inspect;

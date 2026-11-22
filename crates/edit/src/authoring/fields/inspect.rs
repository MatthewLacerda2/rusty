use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use super::table::Field;
use rusty_ecs::World;

pub trait Inspect: Clone + Serialize + DeserializeOwned + 'static {
    const FIELDS: &'static [Field<Self>];
    fn read(world: &World, id: u32) -> Option<Result<Value, String>>;
    fn update(world: &mut World, id: u32, f: &mut dyn FnMut(&mut Self)) -> bool;
    fn draft(live: &Self) -> Self {
        live.clone()
    }
    fn commit(live: &mut Self, draft: Self) {
        *live = draft;
    }
    fn field_type(name: &'static str) -> (bool, Option<&'static str>) {
        super::introspect::field_type::<Self>(name)
    }
}

macro_rules! inspect {
    ($($ty:ty => $get:ident, $get_mut:ident, $fields:expr $(, shown $shown:expr)? $(, field_type $ft:expr)?$(, draft $draft:expr, $commit:expr)?;)*) => {$(
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
            $(
                fn draft(live: &Self) -> Self {
                    ($draft)(live)
                }
                fn commit(live: &mut Self, draft: Self) {
                    ($commit)(live, draft)
                }
            )?
        }
    )*};
}

pub(in crate::authoring::fields) use inspect;

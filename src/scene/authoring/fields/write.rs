//! src/scene/authoring/fields/write.rs — `Component.Set`'s write: a change table onto one
//! component, through the field table's ops (#1369).

use serde_json::{Map, Value};

use super::table::{Access, Inspect, Values};
use super::value::same;
use crate::ecs::World;

/// Which keys of a change table `Set` skips as unchanged.
#[derive(Clone, Copy)]
pub enum Skip<'a> {
    /// A key equal to what the field holds now (a table written by hand).
    Held,
    /// A key equal to what `Get` returned for it (the table came from `Get` on
    /// this entity), or to what the field holds now. So a table held across
    /// ticks writes only its edits, not the sim values it read (a `velocity` the
    /// physics moved since, an animator's `time`).
    SinceGet(&'a Map<String, Value>),
    /// None: every key runs its op (the round-trip tests drive the ops).
    #[cfg(test)]
    Nothing,
}

impl Skip<'_> {
    fn skips(&self, key: &str, value: &Value, held: &Value) -> bool {
        let held = held.get(key).unwrap_or(&Value::Null);
        match self {
            Skip::Held => same(value, held),
            Skip::SinceGet(got) => {
                same(value, got.get(key).unwrap_or(&Value::Null)) || same(value, held)
            }
            #[cfg(test)]
            Skip::Nothing => false,
        }
    }
}

/// Write `changes` onto `id`'s `C`, all or nothing; `Ok(false)` without one.
///
/// Every key is checked first, and the ops run on a copy that replaces the
/// component only when every one succeeded. A key `skip` calls unchanged runs no
/// op, so a table read with `Get`, edited and handed back writes only what was
/// edited (an unchanged Animator `graph` does not restart the state machine), and a
/// read-only field given its own value is not an error. The ops run in the table's order, not the caller's, so a field whose op
/// resets another (a reverb `preset` its `params`) is listed after it and wins.
pub fn write<C: Inspect>(
    world: &mut World,
    id: u32,
    changes: &Map<String, Value>,
    skip: Skip,
) -> Result<bool, String> {
    for key in changes.keys() {
        if !C::FIELDS.iter().any(|f| f.name == key) {
            let names: Vec<&str> = C::FIELDS.iter().map(|f| f.name).collect();
            return Err(format!(
                "unknown field '{key}' (fields: {})",
                names.join(", ")
            ));
        }
    }
    let mut outcome = Ok(());
    let found = C::update(world, id, &mut |live| {
        let mut draft = live.clone();
        outcome = apply(&mut draft, changes, skip);
        if outcome.is_ok() {
            *live = draft;
        }
    });
    outcome.map(|()| found)
}

/// The changed keys of `changes`, through their ops, onto `draft`.
fn apply<C: Inspect>(
    draft: &mut C,
    changes: &Map<String, Value>,
    skip: Skip,
) -> Result<(), String> {
    let held = serialized(draft)?;
    let changed: Map<String, Value> = changes
        .iter()
        .filter(|(k, v)| !skip.skips(k, v, &held))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    run_ops(draft, &changed, &mut |name, e| Err(named(name, e)))
}

/// The one walk over a component's fields, `Set`'s and a load's ([`super::hold`]):
/// call the op of every field `changes` names, in the table's order, with its
/// value there; a group's op once, for any of its fields, each part from
/// `changes`, else as `c` holds it by then. A named read-only field is an error;
/// an op that refuses its value goes to `refused`, which says whether to stop.
pub(super) fn run_ops<C: Inspect>(
    c: &mut C,
    changes: &Map<String, Value>,
    refused: &mut dyn FnMut(&str, String) -> Result<(), String>,
) -> Result<(), String> {
    let mut groups_run: Vec<&str> = Vec::new();
    for field in C::FIELDS {
        let named = changes.get(field.name);
        let outcome = match &field.access {
            Access::ReadOnly(why) if named.is_some() => {
                return Err(format!("field '{}' is read-only: {why}", field.name));
            }
            Access::Op(op) => match named {
                Some(value) => op(c, value.clone()),
                None => continue,
            },
            Access::Group(group) => {
                let first = group.names[0];
                let touched = group.names.iter().any(|n| changes.contains_key(*n));
                if !touched || groups_run.contains(&first) {
                    continue;
                }
                groups_run.push(first);
                let current = serialized(c)?;
                (group.write)(
                    c,
                    &Values {
                        changes,
                        current: &current,
                    },
                )
            }
            Access::ReadOnly(_) => continue,
        };
        if let Err(e) = outcome {
            // A group's error already names the part that failed (`Values::take`), so
            // it goes on unprefixed: naming the group's first part would point at a
            // field the caller never wrote.
            let name = match field.access {
                Access::Group(_) => "",
                _ => field.name,
            };
            refused(name, e)?;
        }
    }
    Ok(())
}

/// `e`, prefixed with the field `name` that raised it (a group's error names its own
/// part, so it comes with an empty `name`).
pub(super) fn named(name: &str, e: String) -> String {
    if name.is_empty() {
        e
    } else {
        format!("{name}: {e}")
    }
}

pub(super) fn serialized<C: Inspect>(c: &C) -> Result<Value, String> {
    serde_json::to_value(c).map_err(|e| e.to_string())
}

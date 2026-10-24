//! src/api/component/write.rs — `Component.Set`'s write: a change table onto one
//! component, through the field table's ops (#1369).

use serde_json::{Map, Value};

use super::table::{Access, Inspect, Values};
use super::value::same;
use crate::ecs::World;

/// Write `changes` onto `id`'s `C`, all or nothing; `Ok(false)` without one.
///
/// Every key is checked first, and the ops run on a copy that replaces the
/// component only when every one succeeded. A key whose value is the one the field
/// already holds is skipped: no op runs, so a table read with `Get`, edited and
/// handed back writes only what was edited (an unchanged Animator `graph` does not
/// restart the state machine), and a read-only field given its own value is not an
/// error. The ops run in the table's order, not the caller's, so a field whose op
/// resets another (a reverb `preset` its `params`) is listed after it and wins.
pub fn write<C: Inspect>(
    world: &mut World,
    id: u32,
    changes: &Map<String, Value>,
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
        outcome = apply(&mut draft, changes);
        if outcome.is_ok() {
            *live = draft;
        }
    });
    outcome.map(|()| found)
}

/// The changed keys of `changes`, through their ops, onto `draft`.
fn apply<C: Inspect>(draft: &mut C, changes: &Map<String, Value>) -> Result<(), String> {
    let held = serialized(draft)?;
    let changed: Map<String, Value> = changes
        .iter()
        .filter(|(k, v)| !same(v, held.get(k.as_str()).unwrap_or(&Value::Null)))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let mut groups_run: Vec<&str> = Vec::new();
    for field in C::FIELDS {
        let named = changed.get(field.name);
        match &field.access {
            Access::ReadOnly(why) if named.is_some() => {
                return Err(format!("field '{}' is read-only: {why}", field.name));
            }
            Access::Op(op) => {
                if let Some(value) = named {
                    op(draft, value.clone()).map_err(|e| format!("{}: {e}", field.name))?;
                }
            }
            Access::Group(group) => {
                let first = group.names[0];
                let touched = group.names.iter().any(|n| changed.contains_key(*n));
                if touched && !groups_run.contains(&first) {
                    groups_run.push(first);
                    let current = serialized(draft)?;
                    let values = Values {
                        changes: &changed,
                        current: &current,
                    };
                    (group.write)(draft, &values)?;
                }
            }
            Access::ReadOnly(_) => {}
        }
    }
    Ok(())
}

fn serialized<C: Inspect>(c: &C) -> Result<Value, String> {
    serde_json::to_value(c).map_err(|e| e.to_string())
}

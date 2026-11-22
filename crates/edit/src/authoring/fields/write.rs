use serde_json::{Map, Value};

use super::table::{Access, Inspect, Values};
use super::value::{changed, same};
use rusty_ecs::World;

#[derive(Clone, Copy)]
pub enum Skip<'a> {
    Held,
    SinceGet(&'a Map<String, Value>),
    Nothing,
}

impl Skip<'_> {
    fn changed(&self, merge: bool, key: &str, value: &Value, held: &Value) -> Option<Value> {
        let held = held.get(key).unwrap_or(&Value::Null);
        let got = match self {
            Skip::Held => None,
            Skip::SinceGet(got) => Some(got.get(key).unwrap_or(&Value::Null)),
            Skip::Nothing => return Some(value.clone()),
        };
        if merge {
            return changed(value, held, got);
        }
        let unchanged = same(value, held) || got.is_some_and(|g| same(value, g));
        (!unchanged).then(|| value.clone())
    }
}

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
        let mut draft = C::draft(live);
        outcome = apply(&mut draft, changes, skip);
        if outcome.is_ok() {
            C::commit(live, draft);
        }
    });
    outcome.map(|()| found)
}

fn apply<C: Inspect>(
    draft: &mut C,
    changes: &Map<String, Value>,
    skip: Skip,
) -> Result<(), String> {
    let held = serialized(draft)?;
    let merges = |key: &str| {
        C::FIELDS
            .iter()
            .any(|f| f.name == key && matches!(f.access, Access::Merge(_)))
    };
    let changed: Map<String, Value> = changes
        .iter()
        .filter_map(|(k, v)| Some((k.clone(), skip.changed(merges(k), k, v, &held)?)))
        .collect();
    run_ops(draft, &changed, &mut |name, e| Err(named(name, e)))
}

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
            Access::Op(op) | Access::Merge(op) => match named {
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
            let name = match field.access {
                Access::Group(_) => "",
                _ => field.name,
            };
            refused(name, e)?;
        }
    }
    Ok(())
}

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

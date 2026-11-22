use std::collections::BTreeMap;

use mlua::{Lua, Table, Value};

use rusty_asset::{MeshColliderKind, SubObjectCollider};

pub(super) fn table(
    lua: &Lua,
    colliders: &BTreeMap<String, SubObjectCollider>,
) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    for (sub, c) in colliders {
        let entry = lua.create_table()?;
        entry.raw_set("kind", kind_name(c.kind))?;
        entry.raw_set("mesh", c.mesh.clone())?;
        t.raw_set(sub.as_str(), entry)?;
    }
    Ok(t)
}

pub(super) fn merge(
    stored: &mut BTreeMap<String, SubObjectCollider>,
    t: &Table,
) -> mlua::Result<bool> {
    let mut merged = stored.clone();
    let changed = merge_into(&mut merged, t)?;
    let looped = rusty_asset::sidecar::looped(&merged);
    if !looped.is_empty() {
        return Err(bad(format!(
            "colliders: the collision meshes of {} would name each other",
            looped.join(", ")
        )));
    }
    *stored = merged;
    Ok(changed)
}

fn merge_into(stored: &mut BTreeMap<String, SubObjectCollider>, t: &Table) -> mlua::Result<bool> {
    let mut changed = false;
    for pair in t.pairs::<String, Value>() {
        let (sub, value) = pair?;
        let new = match value {
            Value::Boolean(false) | Value::Nil => None,
            Value::Table(entry) => Some(entry_of(&sub, &entry)?),
            other => {
                return Err(bad(format!(
                    "colliders.{sub}: want a table or false, got {}",
                    other.type_name()
                )))
            }
        };
        if stored.get(&sub) != new.as_ref() {
            changed = true;
            match new {
                Some(entry) => stored.insert(sub, entry),
                None => stored.remove(&sub),
            };
        }
    }
    Ok(changed)
}

fn entry_of(sub: &str, entry: &Table) -> mlua::Result<SubObjectCollider> {
    let kind = match entry.get::<Option<String>>("kind")?.as_deref() {
        Some("TriMesh") | None => MeshColliderKind::TriMesh,
        Some("Convex") => MeshColliderKind::Convex,
        Some(other) => {
            return Err(bad(format!(
                "colliders.{sub}.kind: unknown {other:?} (want TriMesh or Convex)"
            )))
        }
    };
    let mesh = entry
        .get::<Option<String>>("mesh")?
        .filter(|m| !m.is_empty());
    Ok(SubObjectCollider { kind, mesh }.for_owner(sub))
}

fn kind_name(kind: MeshColliderKind) -> &'static str {
    match kind {
        MeshColliderKind::TriMesh => "TriMesh",
        MeshColliderKind::Convex => "Convex",
    }
}

fn bad(message: String) -> mlua::Error {
    mlua::Error::RuntimeError(format!("SetModelImportSettings: {message}"))
}

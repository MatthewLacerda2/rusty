//! The `colliders` key of `Assets.GetModelImportSettings` / `SetModelImportSettings`
//! (#1960): a model's per-sub-object mesh colliders as its sidecar keeps them
//! (`asset::sidecar::SubObjectCollider`), the model inspector's sub-object rows.

use std::collections::BTreeMap;

use mlua::{Lua, Table, Value};

use crate::asset::{MeshColliderKind, SubObjectCollider};

/// `{ [subObject] = { kind = "TriMesh" | "Convex", mesh? } }`.
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

/// `stored` with `t`'s keys: each `[subObject] = { kind, mesh? }` sets that
/// sub-object's collider (no `mesh`: its own), and `[subObject] = false` removes it.
/// Sub-objects `t` doesn't name keep theirs. Returns whether anything changed.
pub(super) fn merge(
    stored: &mut BTreeMap<String, SubObjectCollider>,
    t: &Table,
) -> mlua::Result<bool> {
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

/// One `{ kind, mesh? }` entry for `sub`; a `mesh` naming `sub` itself is its own.
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

//! Reading a spec table's fields with errors that name them (#1892):
//! `tests[2].distance.to: unknown context 'targte'`, like the animation graph's
//! load errors.

use glam::Vec3;
use mlua::{Table, Value};

/// A Lua error naming the field at `path`.
pub fn fail<T>(path: &str, what: impl std::fmt::Display) -> mlua::Result<T> {
    Err(mlua::Error::RuntimeError(format!("{path}: {what}")))
}

/// `path.key`, the way the errors spell a field.
pub fn at(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

/// Refuse any key of `table` not in `allowed`, so a typo is an error, not a
/// silently ignored field.
pub fn only(table: &Table, path: &str, allowed: &[&str]) -> mlua::Result<()> {
    for pair in table.pairs::<Value, Value>() {
        let (key, _) = pair?;
        let name = match &key {
            Value::String(s) => s.to_str()?.to_string(),
            other => {
                return fail(
                    path,
                    format!("unexpected key of type {}", other.type_name()),
                )
            }
        };
        if !allowed.contains(&name.as_str()) {
            let expected = allowed.join(", ");
            return fail(
                path,
                format!("unknown field '{name}' (expected {expected})"),
            );
        }
    }
    Ok(())
}

/// A finite number at `table[key]`, `default` when absent; `None` default makes
/// it required.
pub fn number(table: &Table, path: &str, key: &str, default: Option<f32>) -> mlua::Result<f32> {
    let here = at(path, key);
    match table.get::<Value>(key)? {
        Value::Nil => default.map_or_else(|| fail(&here, "required"), Ok),
        Value::Integer(i) => Ok(i as f32),
        Value::Number(n) if n.is_finite() => Ok(n as f32),
        Value::Number(_) => fail(&here, "must be finite"),
        other => fail(
            &here,
            format!("expected a number, got {}", other.type_name()),
        ),
    }
}

/// A whole number at least `min` at `table[key]`, `default` when absent.
pub fn count(table: &Table, path: &str, key: &str, default: u32, min: u32) -> mlua::Result<u32> {
    let n = number(table, path, key, Some(default as f32))?;
    if n.fract() != 0.0 || n < min as f32 || n > u32::MAX as f32 {
        return fail(
            &at(path, key),
            format!("expected a whole number ≥ {min}, got {n}"),
        );
    }
    Ok(n as u32)
}

/// A boolean at `table[key]`, `default` when absent.
pub fn boolean(table: &Table, path: &str, key: &str, default: bool) -> mlua::Result<bool> {
    match table.get::<Value>(key)? {
        Value::Nil => Ok(default),
        Value::Boolean(b) => Ok(b),
        other => fail(
            &at(path, key),
            format!("expected a boolean, got {}", other.type_name()),
        ),
    }
}

/// A string at `table[key]`, or `None` when absent.
pub fn text(table: &Table, path: &str, key: &str) -> mlua::Result<Option<String>> {
    match table.get::<Value>(key)? {
        Value::Nil => Ok(None),
        Value::String(s) => Ok(Some(s.to_str()?.to_string())),
        other => fail(
            &at(path, key),
            format!("expected a string, got {}", other.type_name()),
        ),
    }
}

/// A point table: `{x = 1, y = 2, z = 3}` or `{1, 2, 3}`.
pub fn point(value: &Value, path: &str) -> mlua::Result<Vec3> {
    let Value::Table(t) = value else {
        return fail(
            path,
            format!("expected a point {{x, y, z}}, got {}", value.type_name()),
        );
    };
    let named = t.contains_key("x")?;
    let mut xyz = [0.0f32; 3];
    for (k, (name, index)) in [("x", 1), ("y", 2), ("z", 3)].into_iter().enumerate() {
        let v: Value = if named { t.get(name)? } else { t.get(index)? };
        xyz[k] = match v {
            Value::Integer(i) => i as f32,
            Value::Number(n) if n.is_finite() => n as f32,
            _ => {
                return fail(
                    path,
                    format!("expected a point {{x, y, z}} with finite '{name}'"),
                )
            }
        };
    }
    Ok(Vec3::from_array(xyz))
}

/// Whether `value` reads as one point (as opposed to a list of them).
pub fn is_point(value: &Value) -> mlua::Result<bool> {
    Ok(match value {
        Value::Table(t) => {
            t.contains_key("x")? || !matches!(t.get::<Value>(1)?, Value::Table(_) | Value::Nil)
        }
        _ => false,
    })
}

/// A list of point tables.
pub fn points(value: &Value, path: &str) -> mlua::Result<Vec<Vec3>> {
    let Value::Table(t) = value else {
        return fail(
            path,
            format!("expected a list of points, got {}", value.type_name()),
        );
    };
    let mut out = Vec::new();
    for (i, v) in t.sequence_values::<Value>().enumerate() {
        out.push(point(&v?, &format!("{path}[{}]", i + 1))?);
    }
    Ok(out)
}

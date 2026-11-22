use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

pub fn decode<T: DeserializeOwned>(json: Value) -> Result<T, String> {
    let error = match rusty_core::naming::variant_case::decode(json.clone()) {
        Ok(value) => return Ok(value),
        Err(e) => e,
    };
    let mut lists = json;
    if empty_tables_as_lists(&mut lists) {
        rusty_core::naming::variant_case::decode(lists).map_err(|_| error)
    } else {
        Err(error)
    }
}

pub fn over(held: Value, written: Value) -> Value {
    match (held, written) {
        (Value::Object(mut base), Value::Object(top)) => {
            for (key, value) in top {
                let below = base.remove(&key).unwrap_or(Value::Null);
                base.insert(key, over(below, value));
            }
            Value::Object(base)
        }
        (_, written) => written,
    }
}

pub fn merged<T: Serialize + DeserializeOwned>(held: &T, written: Value) -> Result<T, String> {
    let to_json = |v: &T| serde_json::to_value(v).map_err(|e| e.to_string());
    let value: T = decode(over(to_json(held)?, written.clone()))?;
    match unknown(&written, &to_json(&value)?) {
        Some(e) => Err(e),
        None => Ok(value),
    }
}

fn unknown(written: &Value, read: &Value) -> Option<String> {
    let (Value::Object(w), Value::Object(r)) = (written, read) else {
        return None;
    };
    w.iter().find_map(|(key, value)| match r.get(key) {
        None => {
            let names: Vec<&str> = r.keys().map(String::as_str).collect();
            Some(format!(
                "unknown field '{key}' (fields: {})",
                names.join(", ")
            ))
        }
        Some(below) => unknown(value, below).map(|e| format!("{key}.{e}")),
    })
}

pub fn changed(written: &Value, held: &Value, got: Option<&Value>) -> Option<Value> {
    if let (Value::Object(w), Value::Object(h)) = (written, held) {
        let kept: Map<String, Value> = w
            .iter()
            .filter_map(|(key, value)| {
                let held = h.get(key).unwrap_or(&Value::Null);
                let got = got.map(|g| g.get(key).unwrap_or(&Value::Null));
                Some((key.clone(), changed(value, held, got)?))
            })
            .collect();
        return (!kept.is_empty()).then_some(Value::Object(kept));
    }
    let unchanged = same(written, held) || got.is_some_and(|g| same(written, g));
    (!unchanged).then(|| written.clone())
}

fn empty_tables_as_lists(value: &mut Value) -> bool {
    match value {
        Value::Object(map) if map.is_empty() => {
            *value = Value::Array(Vec::new());
            true
        }
        Value::Object(map) => map
            .values_mut()
            .fold(false, |any, v| empty_tables_as_lists(v) | any),
        Value::Array(list) => list
            .iter_mut()
            .fold(false, |any, v| empty_tables_as_lists(v) | any),
        _ => false,
    }
}

pub fn same(written: &Value, held: &Value) -> bool {
    match (written, held) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Array(a), Value::Array(b)) => (0..a.len().max(b.len())).all(|i| {
            same(
                a.get(i).unwrap_or(&Value::Null),
                b.get(i).unwrap_or(&Value::Null),
            )
        }),
        (Value::Object(a), Value::Object(b)) => a.keys().chain(b.keys()).all(|k| {
            same(
                a.get(k).unwrap_or(&Value::Null),
                b.get(k).unwrap_or(&Value::Null),
            )
        }),
        (Value::Object(m), Value::Array(l)) | (Value::Array(l), Value::Object(m)) => {
            m.is_empty() && l.is_empty()
        }
        _ => written == held,
    }
}

pub struct Mask(pub u32);

impl<'de> Deserialize<'de> for Mask {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        i64::deserialize(d).map(|m| Mask(m as u32))
    }
}

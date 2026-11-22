use serde_json::Value;

/// A `null` into an object removes the key. An out-of-range array index is a no-op: a list grows only by its size entry.
pub fn set_pointer(value: &mut Value, pointer: &str, leaf: Value) {
    let tokens: Vec<String> = pointer.split('/').skip(1).map(unescape).collect();
    let Some((last, parents)) = tokens.split_last() else {
        return;
    };
    let mut cursor = value;
    for token in parents {
        cursor = match descend(cursor, token) {
            Some(next) => next,
            None => return,
        };
    }
    match cursor {
        Value::Object(map) if leaf.is_null() => {
            map.remove(last);
        }
        Value::Object(map) => {
            map.insert(last.clone(), leaf);
        }
        Value::Array(items) => {
            if let Some(slot) = last.parse::<usize>().ok().and_then(|i| items.get_mut(i)) {
                *slot = leaf;
            }
        }
        _ => {}
    }
}

fn descend<'a>(cursor: &'a mut Value, token: &str) -> Option<&'a mut Value> {
    match cursor {
        Value::Object(map) => {
            let slot = map
                .entry(token.to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if slot.is_null() {
                *slot = Value::Object(serde_json::Map::new());
            }
            Some(slot)
        }
        Value::Array(items) => token
            .parse::<usize>()
            .ok()
            .and_then(move |i| items.get_mut(i)),
        _ => None,
    }
}

fn unescape(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

//! Leaving images out of an exported glTF: a dropped image takes its textures and the material
//! slots sampling them with it, and later indices are renumbered.

use serde_json::Value;

pub(super) fn images(doc: &mut Value, dropped: &[usize]) {
    if dropped.is_empty() {
        return;
    }
    let images = remove(doc, "images", |i, _| dropped.contains(&i));
    let textures = remove(doc, "textures", |_, texture| {
        let source = texture.get("source").and_then(Value::as_u64);
        source.is_some_and(|s| images.get(s as usize).is_some_and(Option::is_none))
    });
    renumber(doc, "textures", "source", &images);
    if let Some(materials) = doc.get_mut("materials") {
        slots(materials, &textures);
    }
}

fn remove(
    doc: &mut Value,
    key: &str,
    mut drop: impl FnMut(usize, &Value) -> bool,
) -> Vec<Option<usize>> {
    let Some(list) = doc.get_mut(key).and_then(Value::as_array_mut) else {
        return Vec::new();
    };
    let mut next = 0;
    let map: Vec<Option<usize>> = (list.iter().enumerate())
        .map(|(i, entry)| {
            (!drop(i, entry)).then(|| {
                next += 1;
                next - 1
            })
        })
        .collect();
    let mut kept = map.iter();
    list.retain(|_| kept.next().is_some_and(Option::is_some));
    map
}

fn renumber(doc: &mut Value, key: &str, field: &str, map: &[Option<usize>]) {
    let Some(list) = doc.get_mut(key).and_then(Value::as_array_mut) else {
        return;
    };
    for entry in list {
        let old = entry.get(field).and_then(Value::as_u64);
        if let Some(new) = old.and_then(|o| map.get(o as usize).copied().flatten()) {
            entry[field] = Value::from(new);
        }
    }
}

fn slots(value: &mut Value, textures: &[Option<usize>]) {
    match value {
        Value::Object(object) => {
            object.retain(|_, child| {
                let index = child.get("index").and_then(Value::as_u64);
                index.is_none_or(|i| textures.get(i as usize).copied().flatten().is_some())
            });
            for child in object.values_mut() {
                if let Some(i) = child.get("index").and_then(Value::as_u64) {
                    if let Some(new) = textures.get(i as usize).copied().flatten() {
                        child["index"] = Value::from(new);
                    }
                }
                slots(child, textures);
            }
        }
        Value::Array(list) => list.iter_mut().for_each(|v| slots(v, textures)),
        _ => {}
    }
}

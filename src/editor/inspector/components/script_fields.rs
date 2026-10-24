//! src/editor/inspector_script_fields.rs — schema-driven script inspector (#84).
//!
//! Renders a typed egui control per field declared in a Lua script's optional
//! `fields` schema (parsed by `scripting::schema`) — sliders for `[Range]`,
//! tooltips for `[Tooltip]`, section labels for `[Header]`, and a sensible
//! default control inferred from type for plain fields. Edits are written back
//! into the [`ScriptComponent`]'s persisted `values` map.
//!
//! The schema is cached per path and only re-parsed when the file is written: the
//! write log (`core::fs::written`) names it, which the editor's file watch does for
//! an edit made outside the engine (#1327). So the inspector neither spins up a Lua
//! VM nor stats the file every frame. This is editor-only state (the determinism
//! guard covers the sim modules, not `editor`).

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

use crate::core::fs::written::{written_since, Stale};
use crate::scene::{ScriptComponent, ScriptFieldValue};
use crate::scripting::{parse_fields, FieldKind, ScriptField};

/// The parsed schemas by script path, and how far into the write log they are
/// current.
struct SchemaCache {
    seen: u64,
    schemas: HashMap<String, Vec<ScriptField>>,
}

thread_local! {
    static SCHEMA_CACHE: RefCell<SchemaCache> = RefCell::new(SchemaCache {
        seen: written_since(u64::MAX).0,
        schemas: HashMap::new(),
    });
}

/// Draw the field controls for one script. Scripts with no `fields` schema draw
/// nothing, so this is a no-op for plain MonoBehaviour scripts.
pub fn draw_script_fields(ui: &mut egui::Ui, script: &mut ScriptComponent, is_dirty: &mut bool) {
    let fields = cached_schema(&script.path);
    if fields.is_empty() {
        return;
    }
    ui.separator();
    for field in &fields {
        if let Some(header) = &field.header {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(header).strong());
        }
        draw_field(ui, field, &mut script.values, is_dirty);
    }
}

/// One field row: the type-appropriate control bound to the persisted value,
/// with the tooltip (if any) attached as hover text.
fn draw_field(
    ui: &mut egui::Ui,
    field: &ScriptField,
    values: &mut BTreeMap<String, ScriptFieldValue>,
    is_dirty: &mut bool,
) {
    let current = values
        .get(&field.name)
        .cloned()
        .unwrap_or_else(|| field.default.clone());

    let response = match field.kind {
        FieldKind::Number => {
            let mut v = as_number(&current);
            let resp = ui
                .horizontal(|ui| {
                    ui.label(&field.name);
                    match field.range {
                        Some((lo, hi)) => ui.add(egui::Slider::new(&mut v, lo..=hi)),
                        None => ui.add(egui::DragValue::new(&mut v).speed(0.05)),
                    }
                })
                .inner;
            if resp.changed() {
                values.insert(field.name.clone(), ScriptFieldValue::Number(v));
                *is_dirty = true;
            }
            resp
        }
        FieldKind::Boolean => {
            let mut b = as_bool(&current);
            let resp = ui.checkbox(&mut b, &field.name);
            if resp.changed() {
                values.insert(field.name.clone(), ScriptFieldValue::Boolean(b));
                *is_dirty = true;
            }
            resp
        }
        FieldKind::Text => {
            let mut s = as_text(&current);
            let resp = ui
                .horizontal(|ui| {
                    ui.label(&field.name);
                    ui.text_edit_singleline(&mut s)
                })
                .inner;
            if resp.changed() {
                values.insert(field.name.clone(), ScriptFieldValue::Text(s));
                *is_dirty = true;
            }
            resp
        }
    };

    if let Some(tooltip) = &field.tooltip {
        response.on_hover_text(tooltip);
    }
}

fn as_number(v: &ScriptFieldValue) -> f64 {
    match v {
        ScriptFieldValue::Number(n) => *n,
        _ => 0.0,
    }
}

fn as_bool(v: &ScriptFieldValue) -> bool {
    matches!(v, ScriptFieldValue::Boolean(true))
}

fn as_text(v: &ScriptFieldValue) -> String {
    match v {
        ScriptFieldValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

/// Return the parsed schema for `path`, re-parsing only once the file was written.
fn cached_schema(path: &str) -> Vec<ScriptField> {
    SCHEMA_CACHE.with(|cache| {
        let cache = &mut *cache.borrow_mut();
        let (seen, written) = written_since(cache.seen);
        cache.seen = seen;
        if !written.is_empty() {
            let stale = Stale::new(written);
            cache.schemas.retain(|p, _| !stale.matches(p));
        }
        if let Some(fields) = cache.schemas.get(path) {
            return fields.clone();
        }
        // A failed read is a miss retried next frame, never a cached empty schema.
        let Ok(code) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let fields = parse_fields(&code);
        cache.schemas.insert(path.to_string(), fields.clone());
        fields
    })
}

#[cfg(test)]
mod tests {
    use super::cached_schema;
    use crate::core::fs::written::note_written;

    fn script(fields: &str) -> String {
        format!("return {{ fields = {{ {fields} }} }}")
    }

    #[test]
    fn a_schema_is_re_parsed_once_its_file_is_noted() {
        let path = crate::test_temp::dir().join("fields.lua");
        let path = path.to_str().unwrap();
        std::fs::write(path, script("speed = { default = 1 }")).unwrap();
        assert_eq!(cached_schema(path).len(), 1);
        // An edit nobody noted yet: the cached parse stands, no stat per frame.
        std::fs::write(
            path,
            script("speed = { default = 1 }, jump = { default = 2 }"),
        )
        .unwrap();
        assert_eq!(cached_schema(path).len(), 1);
        // The editor's watch (or an engine write) notes it: re-parsed.
        note_written(path);
        assert_eq!(cached_schema(path).len(), 2);
    }

    #[test]
    fn a_failed_read_is_not_cached() {
        let path = crate::test_temp::dir().join("late.lua");
        let path = path.to_str().unwrap();
        assert!(cached_schema(path).is_empty(), "nothing there yet");
        std::fs::write(path, script("speed = { default = 1 }")).unwrap();
        assert_eq!(cached_schema(path).len(), 1, "a miss retries");
    }
}

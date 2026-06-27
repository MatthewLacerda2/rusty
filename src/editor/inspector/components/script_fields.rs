//! src/editor/inspector_script_fields.rs — schema-driven script inspector (#84).
//!
//! Renders a typed egui control per field declared in a Lua script's optional
//! `fields` schema (parsed by `scripting::schema`) — sliders for `[Range]`,
//! tooltips for `[Tooltip]`, section labels for `[Header]`, and a sensible
//! default control inferred from type for plain fields. Edits are written back
//! into the [`ScriptComponent`]'s persisted `values` map.
//!
//! The schema is cached per path and only re-parsed when the file's mtime
//! changes, so the inspector doesn't spin up a Lua VM every frame. This is
//! editor-only state (the determinism guard covers `app`/`scripting`/`physics`/
//! `navigation`, not `editor`).

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::time::SystemTime;

use crate::scene::{ScriptComponent, ScriptFieldValue};
use crate::scripting::{parse_fields, FieldKind, ScriptField};

/// Cached parse for one script path: the file mtime it was parsed at, plus the
/// parsed fields. Re-parsed only when the mtime changes.
type SchemaCacheEntry = (Option<SystemTime>, Vec<ScriptField>);

thread_local! {
    static SCHEMA_CACHE: RefCell<HashMap<String, SchemaCacheEntry>> =
        RefCell::new(HashMap::new());
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

/// Return the parsed schema for `path`, re-parsing only when the file changed.
fn cached_schema(path: &str) -> Vec<ScriptField> {
    let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    SCHEMA_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((cached_mtime, fields)) = cache.get(path) {
            if *cached_mtime == mtime {
                return fields.clone();
            }
        }
        let fields = std::fs::read_to_string(path)
            .map(|code| parse_fields(&code))
            .unwrap_or_default();
        cache.insert(path.to_string(), (mtime, fields.clone()));
        fields
    })
}

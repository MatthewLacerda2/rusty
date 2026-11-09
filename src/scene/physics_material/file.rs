//! src/scene/physics_material/file.rs — a physics material asset's file: plain
//! JSON of `PhysicsMaterial`'s four fields, as the scene document stores the
//! inline one. A missing field reads as its default; a value out of range is
//! clamped, as `Physics.SetPhysicsMaterial` clamps it.
//!
//! stored-format: the `.physmat` physics material asset

use std::path::Path;

use crate::components::PhysicsMaterial;

/// Parse an asset file's text.
pub fn parse(text: &str) -> Result<PhysicsMaterial, String> {
    let m: PhysicsMaterial = serde_json::from_str(text).map_err(|e| e.to_string())?;
    sanitized(m)
}

/// The asset file's text for `material`.
pub fn to_json(material: &PhysicsMaterial) -> String {
    let mut text = serde_json::to_string_pretty(material).unwrap_or_default();
    text.push('\n');
    text
}

/// `m` with its coefficients clamped into range; an error on a non-finite one.
pub(super) fn sanitized(m: PhysicsMaterial) -> Result<PhysicsMaterial, String> {
    if !m.friction.is_finite() || !m.bounciness.is_finite() {
        return Err("friction and bounciness must be finite".into());
    }
    Ok(PhysicsMaterial {
        friction_combine: m.friction_combine,
        bounce_combine: m.bounce_combine,
        ..PhysicsMaterial::new(m.friction, m.bounciness)
    })
}

pub(super) fn read(path: &Path) -> Result<PhysicsMaterial, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse(&text)
}

pub(super) fn write(path: &Path, material: &PhysicsMaterial) -> Result<(), String> {
    let shown = path.display();
    if path.extension().and_then(|e| e.to_str()) != Some(super::EXTENSION) {
        return Err(format!("'{shown}' must end in .{}", super::EXTENSION));
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("'{shown}': {e}"))?;
    }
    let text = to_json(material);
    crate::core::fs::written::write(path, text.as_bytes()).map_err(|e| format!("'{shown}': {e}"))
}

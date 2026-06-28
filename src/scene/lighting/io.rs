//! src/scene/lighting_io.rs — the `<scene>.lighting.json` SH sidecar.
//!
//! Mirrors the scene ↔ sidecar split: the scene document stores probe POSITIONS +
//! grid layout (human-diffable references/values); the heavy baked SH lives here, in
//! a sidecar keyed by probe index. This keeps the `.scene` file small and readable
//! while the regenerable SH stays out of git's way — the same rationale as not
//! storing GPU buffers in the scene.
//!
//! Save writes the sidecar next to the scene (`foo.scene` → `foo.scene.lighting.json`)
//! ONLY when probes carry baked SH; load merges it back onto the in-memory probes by
//! index. A missing sidecar is not an error — the probes simply load with zero SH
//! (e.g. positions placed but never baked).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::scene::lighting::sh::Sh9;
use crate::scene::Scene;

/// Suffix appended to a scene path to locate its lighting sidecar.
pub const LIGHTING_SIDECAR_SUFFIX: &str = ".lighting.json";

/// The on-disk sidecar: the baked SH for each probe, index-aligned with the scene's
/// `ProbeVolume::probes`. Deliberately minimal — just the coefficients.
#[derive(Serialize, Deserialize, Default)]
pub struct LightingData {
    /// One `Sh9` per probe, in probe order. Length should match the scene's probe
    /// count; a mismatch tolerates by merging up to the shorter length.
    pub probe_sh: Vec<Sh9>,
}

/// The sidecar path for a scene path: `foo.scene` → `foo.scene.lighting.json`.
pub fn sidecar_path(scene_path: &str) -> PathBuf {
    let mut s = scene_path.to_string();
    s.push_str(LIGHTING_SIDECAR_SUFFIX);
    PathBuf::from(s)
}

/// Pull the baked SH out of a scene's probes into a serializable sidecar.
pub fn extract_lighting(scene: &Scene) -> LightingData {
    LightingData {
        probe_sh: scene.probes.probes.iter().map(|p| p.sh).collect(),
    }
}

/// Apply a loaded sidecar's SH back onto the scene's probes, by index. Extra entries
/// on either side are ignored (the shorter length wins) so a stale sidecar can't
/// panic the load.
pub fn apply_lighting(scene: &mut Scene, data: &LightingData) {
    for (probe, sh) in scene.probes.probes.iter_mut().zip(data.probe_sh.iter()) {
        probe.sh = *sh;
    }
}

/// Write the lighting sidecar next to `scene_path`, but only if at least one probe
/// carries non-zero SH. When no probe is baked the sidecar is removed (if present) so
/// stale data can't linger. Errors are returned as strings, matching scene I/O.
pub fn save_lighting_sidecar(scene: &Scene, scene_path: &str) -> Result<(), String> {
    let path = sidecar_path(scene_path);
    let has_baked = scene.probes.probes.iter().any(|p| p.sh != Sh9::zero());
    if !has_baked {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("Failed to remove sidecar: {}", e))?;
        }
        return Ok(());
    }
    let data = extract_lighting(scene);
    let json = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("Failed to serialize lighting sidecar: {}", e))?;
    std::fs::write(&path, json).map_err(|e| format!("Failed to write lighting sidecar: {}", e))
}

/// Load the lighting sidecar for `scene_path` onto the scene's probes, if it exists.
/// A missing sidecar is a no-op (probes keep zero SH). Returns an error only on a
/// present-but-corrupt sidecar.
pub fn load_lighting_sidecar(scene: &mut Scene, scene_path: &str) -> Result<(), String> {
    let path = sidecar_path(scene_path);
    if !Path::new(&path).exists() {
        return Ok(());
    }
    let json = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read lighting sidecar: {}", e))?;
    let data: LightingData = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to deserialize lighting sidecar: {}", e))?;
    apply_lighting(scene, &data);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn sidecar_path_appends_suffix() {
        assert_eq!(
            sidecar_path("a/foo.scene"),
            PathBuf::from("a/foo.scene.lighting.json")
        );
    }

    #[test]
    fn extract_apply_round_trips() {
        let mut scene = Scene::default();
        scene.probes.add_probe(Vec3::ZERO);
        scene.probes.add_probe(Vec3::X);
        scene.probes.probes[1].sh.coeffs[0] = [1.0, 2.0, 3.0];
        let data = extract_lighting(&scene);

        let mut other = Scene::default();
        other.probes.add_probe(Vec3::ZERO);
        other.probes.add_probe(Vec3::X);
        apply_lighting(&mut other, &data);
        assert_eq!(other.probes.probes[1].sh.coeffs[0], [1.0, 2.0, 3.0]);
    }
}

//! src/scene/lighting/io.rs — the `<scene>.lighting.json` SH sidecar (and, after a
//! `Shadowmask` bake, each probe's shadowmask occlusion beside its SH, #1695).
//!
//! Mirrors the scene ↔ sidecar split: the scene document stores probe POSITIONS +
//! grid layout (human-diffable references/values); the heavy baked SH lives here, in
//! a sidecar keyed by probe index. This keeps the `.scene` file small and readable
//! while the regenerable SH stays out of git's way — the same rationale as not
//! storing GPU buffers in the scene.
//!
//! Save writes the sidecar next to the scene (`foo.scene` → `foo.scene.lighting.json`)
//! ONLY when probes carry baked SH, and removes it when none do; load merges it back
//! onto the in-memory probes by index. It is half of the scene document, so a save
//! writes it with the document, always, never by the bake files' keep-newer rule
//! (`scene::io::BakeFiles`, #1982). The sidecar carries a key of the probe positions
//! it was saved with ([`ProbeKey`]): one whose key disagrees with the document's
//! probes (another process re-placed them and the document was saved over since) is
//! refused on load with a warning, so SH never lands on the wrong probe. A missing
//! sidecar is not an error — the probes simply load with zero SH (e.g. positions
//! placed but never baked).
//!
//! stored-format: the `<scene>.lighting.json` SH sidecar

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::scene::lighting::sh::Sh9;
use crate::scene::Scene;

/// Suffix appended to a scene path to locate its lighting sidecar.
const LIGHTING_SIDECAR_SUFFIX: &str = ".lighting.json";

/// The on-disk sidecar: the baked SH for each probe, index-aligned with the scene's
/// `ProbeVolume::probes`. Deliberately minimal — just the coefficients.
#[derive(Serialize, Deserialize, Default)]
pub struct LightingData {
    /// The probe positions this SH was baked for; `None` (a file without it) is
    /// refused like a mismatch.
    #[serde(default)]
    pub probes: Option<ProbeKey>,
    /// One `Sh9` per probe, in probe order. Length should match the scene's probe
    /// count; a mismatch tolerates by merging up to the shorter length.
    pub probe_sh: Vec<Sh9>,
    /// One shadowmask occlusion per probe (#1695, `Probe::occlusion`), in probe
    /// order; left out when every probe is unoccluded (no `Shadowmask` bake), and
    /// read as unoccluded when missing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probe_occlusion: Vec<[f32; 4]>,
}

/// What a sidecar's SH is index-aligned to: the probe count and a hash of every
/// position's bits, in order.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeKey {
    pub count: usize,
    pub hash: u64,
}

impl ProbeKey {
    /// The key of `scene`'s probes.
    pub fn of(scene: &Scene) -> Self {
        let probes = scene.probes.probes();
        let bits = probes.iter().flat_map(|p| p.position.to_array());
        let bytes: Vec<u8> = bits.flat_map(f32::to_le_bytes).collect();
        Self {
            count: probes.len(),
            hash: crate::core::hash::content(&bytes),
        }
    }
}

/// The sidecar path for a scene path: `foo.scene` → `foo.scene.lighting.json`.
pub fn sidecar_path(scene_path: &str) -> PathBuf {
    let mut s = scene_path.to_string();
    s.push_str(LIGHTING_SIDECAR_SUFFIX);
    PathBuf::from(s)
}

/// Pull the baked SH out of a scene's probes into a serializable sidecar.
pub fn extract_lighting(scene: &Scene) -> LightingData {
    let probes = scene.probes.probes();
    let masked = probes.iter().any(|p| p.occlusion != glam::Vec4::ONE);
    LightingData {
        probes: Some(ProbeKey::of(scene)),
        probe_sh: probes.iter().map(|p| p.sh).collect(),
        probe_occlusion: match masked {
            true => probes.iter().map(|p| p.occlusion.to_array()).collect(),
            false => Vec::new(),
        },
    }
}

/// Apply a loaded sidecar's SH back onto the scene's probes, by index. Extra entries
/// on either side are ignored (the shorter length wins) so a stale sidecar can't
/// panic the load.
pub fn apply_lighting(scene: &mut Scene, data: &LightingData) {
    for (probe, sh) in scene
        .probes
        .probes_mut()
        .iter_mut()
        .zip(data.probe_sh.iter())
    {
        probe.sh = *sh;
    }
    let occlusion = data
        .probe_occlusion
        .iter()
        .map(|o| glam::Vec4::from_array(*o));
    let mut occlusion = occlusion.chain(std::iter::repeat(glam::Vec4::ONE));
    for probe in scene.probes.probes_mut() {
        probe.occlusion = occlusion.next().unwrap_or(glam::Vec4::ONE);
    }
}

/// Write the baked PVS beside `scene_path` (`occlusion::save_sidecar`, #1714), by
/// the bake files' rule (`scene::io::BakeFiles`, #1982), and the lighting sidecar
/// next to it ([`save_sh_sidecar`]): a whole scene save. Errors are returned as
/// strings, matching scene I/O.
pub fn save_lighting_sidecar(scene: &Scene, scene_path: &str) -> Result<(), String> {
    super::occlusion::save_sidecar(scene, scene_path)?;
    save_sh_sidecar(scene, scene_path)
}

/// Write the lighting sidecar beside `scene_path`, with the document: written when
/// any probe is baked, removed when none is, so stale data can't linger. Generate
/// Lighting's lighting save writes only this one; its PVS is written once baked.
pub fn save_sh_sidecar(scene: &Scene, scene_path: &str) -> Result<(), String> {
    let path = sidecar_path(scene_path);
    let has_baked = (scene.probes.probes().iter())
        .any(|p| p.sh != Sh9::zero() || p.occlusion != glam::Vec4::ONE);
    if !has_baked {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("Failed to remove sidecar: {}", e))?;
        }
        return Ok(());
    }
    let json = serde_json::to_string_pretty(&extract_lighting(scene))
        .map_err(|e| format!("Failed to serialize lighting sidecar: {}", e))?;
    // lint: allow(asset-write) the SH sidecar is half of the scene document, read whole on load
    crate::core::fs::write_atomic(&path, json.as_bytes())
        .map_err(|e| format!("Failed to write lighting sidecar: {}", e))
}

/// Load the lighting sidecar for `scene_path` onto the scene's probes, if it exists
/// and was saved for these probes, and the baked PVS beside it
/// (`occlusion::load_sidecar`, #1714). A missing sidecar is a no-op (probes keep
/// zero SH), and one saved for other probes is left unapplied with a warning.
/// Returns an error only on a present-but-corrupt sidecar.
pub fn load_lighting_sidecar(scene: &mut Scene, scene_path: &str) -> Result<(), String> {
    super::occlusion::load_sidecar(scene, scene_path);
    let path = sidecar_path(scene_path);
    let json = match std::fs::read_to_string(&path) {
        Ok(json) => json,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("Failed to read lighting sidecar: {}", e)),
    };
    let data: LightingData = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to deserialize lighting sidecar: {}", e))?;
    if data.probes != Some(ProbeKey::of(scene)) {
        let why = match data.probes {
            None => "by an older engine, with no probe key",
            Some(_) => {
                "for other probe positions than the scene's (they were re-placed and \
                 the scene saved over since?)"
            }
        };
        log::warn!(
            "[Lighting] {} was saved {why}: its SH is not applied, and the probes are \
             unbaked until Generate Lighting",
            path.display()
        );
        return Ok(());
    }
    apply_lighting(scene, &data);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::paths::native;
    use glam::Vec3;

    #[test]
    fn sidecar_path_appends_suffix() {
        assert_eq!(
            sidecar_path("a/foo.scene"),
            native("a/foo.scene.lighting.json")
        );
    }

    #[test]
    fn extract_apply_round_trips() {
        let mut scene = Scene::default();
        scene.probes.add_probe(Vec3::ZERO);
        scene.probes.add_probe(Vec3::X);
        scene.probes.probes_mut()[1].sh.coeffs[0] = [1.0, 2.0, 3.0];
        let data = extract_lighting(&scene);

        let mut other = Scene::default();
        other.probes.add_probe(Vec3::ZERO);
        other.probes.add_probe(Vec3::X);
        apply_lighting(&mut other, &data);
        assert_eq!(other.probes.probes()[1].sh.coeffs[0], [1.0, 2.0, 3.0]);
    }

    #[test]
    fn a_probe_with_only_occlusion_is_saved() {
        let dir = crate::test_temp::dir().join("rusty_probe_occlusion_sidecar");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("o.scene").to_string_lossy().into_owned();
        let mut scene = Scene::default();
        scene.probes.add_probe(Vec3::ZERO);
        scene.probes.probes_mut()[0].occlusion = glam::Vec4::new(0.0, 1.0, 1.0, 1.0);
        save_lighting_sidecar(&scene, &path).unwrap();
        let mut back = Scene::default();
        back.probes.add_probe(Vec3::ZERO);
        load_lighting_sidecar(&mut back, &path).unwrap();
        assert_eq!(back.probes.probes()[0].occlusion.x, 0.0);
        std::fs::remove_dir_all(&dir).ok();
    }
}

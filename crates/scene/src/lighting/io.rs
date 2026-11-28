//! stored-format: the `<scene>.lighting.json` SH sidecar

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::lighting::sh::Sh9;
use crate::Scene;

const LIGHTING_SIDECAR_SUFFIX: &str = ".lighting.json";

#[derive(Serialize, Deserialize, Default)]
pub struct LightingData {
    #[serde(default)]
    pub probes: Option<ProbeKey>,
    pub probe_sh: Vec<Sh9>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probe_occlusion: Vec<[f32; 4]>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeKey {
    pub count: usize,
    pub hash: u64,
}

impl ProbeKey {
    pub fn of(scene: &Scene) -> Self {
        Self::of_set(&scene.probes)
    }

    /// One loaded scene's set: each scene's sidecar is keyed by its own set alone.
    pub fn of_set(set: &crate::lighting::probe::ProbeVolume) -> Self {
        let probes = set.probes();
        let bits = probes.iter().flat_map(|p| p.position.to_array());
        let bytes: Vec<u8> = bits.flat_map(f32::to_le_bytes).collect();
        Self {
            count: probes.len(),
            hash: rusty_core::hash::content(&bytes),
        }
    }
}

pub fn sidecar_path(scene_path: &str) -> PathBuf {
    let mut s = scene_path.to_string();
    s.push_str(LIGHTING_SIDECAR_SUFFIX);
    PathBuf::from(s)
}

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

pub fn save_lighting_sidecar(scene: &Scene, scene_path: &str) -> Result<(), String> {
    super::occlusion::save_sidecar(scene, scene_path)?;
    save_sh_sidecar(scene, scene_path)
}

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
    rusty_core::fs::write_atomic(&path, json.as_bytes())
        .map_err(|e| format!("Failed to write lighting sidecar: {}", e))
}

pub fn load_lighting_sidecar(scene: &mut Scene, scene_path: &str) -> Result<(), String> {
    apply_lighting_sidecar(scene, read_lighting_sidecar(scene_path))
}

/// Read from the files alone and put on a scene later, so a load does its work before
/// it touches the live scene.
pub struct LightingFile {
    occlusion: super::occlusion::OcclusionFile,
    path: std::path::PathBuf,
    read: Result<Option<LightingData>, String>,
}

impl LightingFile {
    /// Whether the baked probe lighting file sits beside the scene.
    pub fn has_probes(&self) -> bool {
        !matches!(self.read, Ok(None))
    }

    /// Whether the baked occlusion (PVS) file sits beside the scene.
    pub fn has_occlusion(&self) -> bool {
        self.occlusion.exists()
    }
}

pub fn read_lighting_sidecar(scene_path: &str) -> LightingFile {
    let occlusion = super::occlusion::read_sidecar(scene_path);
    let path = sidecar_path(scene_path);
    let read = match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json)
            .map(Some)
            .map_err(|e| format!("Failed to deserialize lighting sidecar: {}", e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Failed to read lighting sidecar: {}", e)),
    };
    LightingFile {
        occlusion,
        path,
        read,
    }
}

pub fn apply_lighting_sidecar(scene: &mut Scene, file: LightingFile) -> Result<(), String> {
    super::occlusion::apply_sidecar(scene, file.occlusion);
    let Some(data) = file.read? else {
        return Ok(());
    };
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
            file.path.display()
        );
        return Ok(());
    }
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
        let dir = rusty_test_support::temp::dir().join("rusty_probe_occlusion_sidecar");
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

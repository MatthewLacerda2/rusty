//! stored-format: `project.rusty`, the project file
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::engine::commit::{engine_commit, engine_order, UNKNOWN_COMMIT};
use super::engine::{base, EngineCheck};
use crate::application::BuildSettings;
use crate::settings::audio::AudioSettings;
use crate::settings::decals::DecalLimits;
use crate::settings::demos::DemoSettings;
use crate::settings::graphics_options::GraphicsOptions;
use crate::settings::logs::LogSettings;
use crate::settings::quality::QualityPresets;
use crate::settings::render_scale::ProjectGraphics;

pub const PROJECT_FILE: &str = "project.rusty";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectFile {
    pub engine_commit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_order: Option<u64>,
    pub build: BuildSettings,
    pub graphics_options: GraphicsOptions,
    pub graphics: ProjectGraphics,
    pub quality: QualityPresets,
    pub decals: DecalLimits,
    pub demos: DemoSettings,
    pub audio: AudioSettings,
    pub logs: LogSettings,
}

impl Default for ProjectFile {
    fn default() -> Self {
        Self {
            engine_commit: engine_commit().to_string(),
            engine_order: engine_order(),
            build: BuildSettings::default(),
            graphics_options: GraphicsOptions::default(),
            graphics: ProjectGraphics::default(),
            quality: QualityPresets::default(),
            decals: DecalLimits::default(),
            demos: DemoSettings::default(),
            audio: AudioSettings::default(),
            logs: LogSettings::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectSettings {
    pub build: BuildSettings,
    pub graphics_options: GraphicsOptions,
    pub graphics: ProjectGraphics,
    pub quality: QualityPresets,
    pub decals: DecalLimits,
    pub demos: DemoSettings,
    pub audio: AudioSettings,
    pub logs: LogSettings,
}

impl From<ProjectFile> for ProjectSettings {
    fn from(file: ProjectFile) -> Self {
        Self {
            build: file.build,
            graphics_options: file.graphics_options,
            graphics: file.graphics,
            quality: file.quality,
            decals: file.decals,
            demos: file.demos,
            audio: file.audio,
            logs: file.logs,
        }
    }
}

impl ProjectFile {
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("Failed to parse {PROJECT_FILE}: {e}"))
    }

    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_default();
        text.push('\n');
        text
    }

    pub fn read(path: &Path) -> Result<Option<Self>, String> {
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
        Self::from_json(&text).map(Some)
    }

    pub fn load(root: &Path) -> Result<Self, String> {
        Ok(Self::read(&root.join(PROJECT_FILE))?.unwrap_or_default())
    }

    pub fn write(&self, path: &Path) -> Result<(), String> {
        let text = self.to_json();
        if std::fs::read_to_string(path).is_ok_and(|old| old == text) {
            return Ok(());
        }
        // lint: allow(asset-write) project settings, not read by a path-keyed cache
        crate::fs::write_atomic(path, text.as_bytes())
            .map_err(|e| format!("Failed to write {}: {e}", path.display()))
    }
}

pub(super) fn record(root: &Path) -> Result<(EngineCheck, ProjectFile, Vec<String>), String> {
    let path = root.join(PROJECT_FILE);
    let existed = path.exists();
    let mut file = ProjectFile::load(root)?;
    let check = EngineCheck::against(Some(&file.engine_commit), file.engine_order);
    let mut lines = Vec::new();
    if let Some(name) = root.file_name().filter(|_| !existed) {
        file.build.product_name = name.to_string_lossy().into_owned();
    }
    if engine_commit() != UNKNOWN_COMMIT || file.engine_commit.is_empty() {
        if engine_order().is_some() || base(&file.engine_commit) != base(engine_commit()) {
            file.engine_order = engine_order();
        }
        file.engine_commit = engine_commit().to_string();
    }
    file.write(&path)?;
    if !existed {
        lines.push(format!("created {PROJECT_FILE}"));
    }
    Ok((check, file, lines))
}

pub(super) fn inspect(root: &Path) -> Result<(EngineCheck, ProjectFile), String> {
    let file = ProjectFile::load(root)?;
    let check = EngineCheck::against(Some(&file.engine_commit), file.engine_order);
    Ok((check, file))
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod file_tests;

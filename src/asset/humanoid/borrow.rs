//! A model's borrowed clips (#1747): each `clips_from` model's clips,
//! retargeted onto every skinned sub-mesh of the importing model.

use std::path::Path;

use super::{retarget, BoneMap, HumanoidSettings};
use crate::asset::anim_data::AnimationClip;
use crate::asset::mesh_data::{ImportedAsset, SkinData};
use crate::asset::ImportError;

/// A source model's rig and clips: its first skinned sub-mesh that has clips.
pub struct Source {
    pub path: String,
    pub skin: SkinData,
    pub map: BoneMap,
    pub clips: Vec<AnimationClip>,
}

impl Source {
    /// The rig and clips of `asset` (imported from `path` with `settings`), or
    /// `None` when no sub-mesh of it is skinned and animated.
    pub fn of(path: &str, asset: ImportedAsset, settings: &HumanoidSettings) -> Option<Self> {
        let sub =
            (asset.sub_meshes.into_iter()).find(|s| s.skin.is_some() && !s.clips.is_empty())?;
        let skin = sub.skin?;
        let map = BoneMap::resolve(&skin, settings);
        warn_problems(path, &map);
        Some(Self {
            path: path.to_string(),
            skin,
            map,
            clips: sub.clips,
        })
    }
}

/// Append to each skinned sub-mesh of `asset` (imported from `path`) the clips
/// of the models `settings.clips_from` names, retargeted onto its rig. `load`
/// reads a source model with its own settings (never its own borrowed clips, so
/// two models borrowing from each other end there). A clip whose name the
/// sub-mesh already has is skipped: its own clips, and earlier sources, win.
/// What cannot be read or mapped is warned of and left out.
pub fn borrow_clips(
    path: &Path,
    asset: &mut ImportedAsset,
    settings: &HumanoidSettings,
    load: impl Fn(&str) -> Result<Option<Source>, ImportError>,
) {
    if settings.clips_from.is_empty() {
        return;
    }
    let shown = path.display().to_string();
    let sources: Vec<Source> = (settings.clips_from.iter())
        .filter_map(|from| match load(from) {
            Ok(Some(source)) => Some(source),
            Ok(None) => {
                log::warn!(
                    "[Asset] {shown}: '{from}' has no animated skeleton to borrow clips from"
                );
                None
            }
            Err(e) => {
                log::warn!("[Asset] {shown}: cannot borrow clips from '{from}': {e}");
                None
            }
        })
        .collect();
    let mut warned = false;
    for sub in &mut asset.sub_meshes {
        let Some(skin) = &sub.skin else { continue };
        let map = BoneMap::resolve(skin, settings);
        if !warned {
            warn_problems(&shown, &map);
            warned = true;
        }
        for source in &sources {
            for clip in &source.clips {
                if sub.clips.iter().any(|c| c.name == clip.name) {
                    log::warn!(
                        "[Asset] {shown}: clip '{}' from '{}' skipped: the model already has one of that name",
                        clip.name,
                        source.path
                    );
                    continue;
                }
                let moved = retarget(clip, (&source.skin, &source.map), (skin, &map));
                sub.clips.push(moved);
            }
        }
    }
}

/// Say once what resolving `map` for the model at `path` found wrong.
fn warn_problems(path: &str, map: &BoneMap) {
    if !map.problems.is_empty() {
        log::warn!("[Asset] {path}: humanoid map: {}", map.problems.join("; "));
    }
}

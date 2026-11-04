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
        if let Some(lost) = root_travel_lost(&skin, &map, &sub.clips) {
            log::warn!("[Asset] {path}: {lost}");
        }
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
            if !source.map.shares_any(&map) {
                log::warn!(
                    "[Asset] {shown}: no humanoid bone of '{}' maps onto this rig; its clips are left out",
                    source.path
                );
                continue;
            }
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

/// What a borrowed clip loses when a joint above `skin`'s hips (a UE-style
/// `root`) carries travel in one of `clips`: the retarget moves only the
/// hips, so the clip plays in place on every rig that borrows it.
pub(crate) fn root_travel_lost(
    skin: &SkinData,
    map: &BoneMap,
    clips: &[AnimationClip],
) -> Option<String> {
    let mut up = skin.parents.get(map.slot("Hips")?).copied().flatten();
    // Bounded by the joint count, so a malformed parent loop ends.
    for _ in 0..skin.parents.len() {
        let joint = up?;
        let moving = clips.iter().find(|c| {
            (c.tracks.get(joint))
                .is_some_and(|t| t.translation.values.windows(2).any(|w| w[0] != w[1]))
        });
        if let Some(clip) = moving {
            return Some(format!(
                "'{}' above the hips moves in clip '{}': a borrowed clip carries only the hips' travel, so it plays in place (key the travel on the hips)",
                skin.name(joint),
                clip.name
            ));
        }
        up = skin.parents.get(joint).copied().flatten();
    }
    None
}

/// Say once what resolving `map` for the model at `path` found wrong.
fn warn_problems(path: &str, map: &BoneMap) {
    if !map.problems.is_empty() {
        log::warn!("[Asset] {path}: humanoid map: {}", map.problems.join("; "));
    }
}

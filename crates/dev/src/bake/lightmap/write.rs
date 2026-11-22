//! stored-format: the lightmap pages a bake writes into `<scene>.lightmaps/`

use std::collections::BTreeSet;
use std::path::Path;

use rusty_lighting::lightmap::{BakeProgress, Lightmap};
use rusty_scene::lighting::lightmap_set::LightmapSet;
use rusty_scene::Scene;

use super::spill::Spilled;
use super::{encode_pages, lightmap_dir, EncodedPages};

pub(super) const FINAL: &str = "lightmap";
/// Starts with `FINAL`'s `lightmap_`, so a finished bake's `clear_pages` removes previews too.
pub(super) const PREVIEW: &str = "lightmap_preview";

/// A test hook: `limits_tests` writes maps past the page limit through it.
pub fn apply_lightmaps(
    scene: &mut Scene,
    scene_path: &str,
    maps: Vec<Lightmap>,
) -> Result<usize, String> {
    let page = scene.lighting_settings.lightmaps.page();
    let encoded = encode_pages(maps, page, &BakeProgress::default(), scene_path)?;
    apply_encoded(scene, scene_path, &encoded.ok_or("never cancelled")?)
}

pub fn apply_encoded(
    scene: &mut Scene,
    scene_path: &str,
    encoded: &EncodedPages,
) -> Result<usize, String> {
    let dir = lightmap_dir(scene_path);
    if encoded.maps == 0 {
        // No page holds a light's bounce, so `lights` stays empty; the channels still go in.
        let mut set = LightmapSet::default();
        set.channels = encoded.shadowmask.channels.clone();
        set.fell_back = encoded.shadowmask.fell_back.clone();
        set.record_taken(&encoded.lights);
        warn_fell_back(&set);
        scene.lightmaps = set.into();
        warn_unremoved(clear_pages(&dir, FINAL, &BTreeSet::new()));
        return Ok(0);
    }
    let size = encoded.page_size;
    let set = match write_set(&dir, FINAL, encoded) {
        Ok(set) => set,
        Err(e) => {
            let mut keep = stems(&scene.lightmaps);
            keep.extend(
                scene
                    .lightmaps
                    .shown_over
                    .as_deref()
                    .map_or_else(BTreeSet::new, stems),
            );
            warn_unremoved(clear_pages(&dir, FINAL, &keep));
            return Err(e);
        }
    };
    warn_fell_back(&set);
    let bytes = encoded.gpu_bytes;
    log::info!(
        "[LightmapBake] {} lightmaps in {} page(s) of {size}²{}, {:.1} MiB on the GPU, in {}{}",
        encoded.maps,
        set.pages.len(),
        match (set.directions.is_empty(), set.masks.is_empty()) {
            (true, true) => "",
            (false, true) => " (directional)",
            (true, false) => " (shadowmask)",
            (false, false) => " (directional, shadowmask)",
        },
        bytes as f64 / (1024.0 * 1024.0),
        dir.display(),
        crate::memory::rss_peak_bytes()
            .map(|b| format!("; process peak {}", crate::bake::progress::format::gb(b)))
            .unwrap_or_default()
    );
    let keep = stems(&set);
    scene.lightmaps = set.into();
    warn_unremoved(clear_pages(&dir, FINAL, &keep));
    Ok(encoded.maps)
}

fn warn_unremoved(cleared: Result<(), String>) {
    if let Err(e) = cleared {
        log::warn!("[LightmapBake] remove an old lightmap page: {e}");
    }
}

pub(super) fn stems(set: &LightmapSet) -> BTreeSet<String> {
    let paths = set.pages.iter().chain(&set.directions).chain(&set.masks);
    let stem = |p: &String| Some(Path::new(p).file_stem()?.to_string_lossy().into_owned());
    paths.filter_map(stem).collect()
}

pub(super) fn write_set(
    dir: &Path,
    prefix: &str,
    encoded: &EncodedPages,
) -> Result<LightmapSet, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("lightmap folder: {e}"))?;
    let mut set = LightmapSet::default();
    for (k, (ktx2, png)) in encoded.colour.iter().enumerate() {
        let stem = dir.join(format!("{prefix}_{k}_{:08x}", ktx2.tag()));
        link(png, &stem.with_extension("png"))?;
        set.pages.push(link(ktx2, &stem.with_extension("ktx2"))?);
    }
    for (k, page) in encoded.directions.iter().enumerate() {
        let path = dir.join(format!("{prefix}_dir_{k}_{:08x}.ktx2", page.tag()));
        set.directions.push(link(page, &path)?);
    }
    for (k, png) in encoded.masks.iter().enumerate() {
        let path = dir.join(format!("{prefix}_mask_{k}_{:08x}.png", png.tag()));
        set.masks.push(link(png, &path)?);
    }
    set.extend(encoded.entries.iter().cloned());
    set.lights = encoded.lights.keys().copied().collect();
    set.channels = encoded.shadowmask.channels.clone();
    set.fell_back = encoded.shadowmask.fell_back.clone();
    set.record_taken(&encoded.lights);
    Ok(set)
}

fn link(file: &Spilled, path: &Path) -> Result<String, String> {
    if !there(path, file.len()) {
        file.link(path)?;
        rusty_core::fs::written::note_written(&path.to_string_lossy());
    }
    Ok(rusty_core::paths::to_slash(path))
}

pub(super) fn write_png(dir: &Path, prefix: &str, k: usize, png: &[u8]) -> Result<String, String> {
    let path = dir.join(format!("{prefix}_{k}_{:08x}.png", content_tag(png)));
    if !there(&path, png.len() as u64) {
        std::fs::write(&path, png).map_err(|e| format!("write {}: {e}", path.display()))?;
        rusty_core::fs::written::note_written(&path.to_string_lossy());
    }
    Ok(rusty_core::paths::to_slash(&path))
}

fn warn_fell_back(set: &LightmapSet) {
    if set.fell_back.is_empty() {
        return;
    }
    log::warn!(
        "[LightmapBake] {} Mixed light(s) overlap 4 others and were baked as Baked \
         (no shadowmask channel left): entities {:?}. Turn some to Baked, or shrink \
         their range.",
        set.fell_back.len(),
        set.fell_back
    );
}

/// The name carries the content tag, so a file of that name and length is this page already.
fn there(path: &Path, len: u64) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() == len)
}

pub(super) fn content_tag(bytes: &[u8]) -> u32 {
    fold_tag(rusty_core::hash::content(bytes))
}

pub(super) fn fold_tag(hash: u64) -> u32 {
    (hash ^ (hash >> 32)) as u32
}

/// Only `.png`: every preview page is one (`encoder::write_preview`).
pub(super) fn remove_stems(dir: &Path, gone: &BTreeSet<String>) -> Result<(), String> {
    for stem in gone {
        let path = dir.join(format!("{stem}.png"));
        match std::fs::remove_file(&path) {
            Ok(()) => {
                rusty_core::fs::written::note_written(&path.to_string_lossy());
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("remove {}: {e}", path.display())),
        }
    }
    Ok(())
}

pub(super) fn clear_pages(dir: &Path, prefix: &str, keep: &BTreeSet<String>) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let page = name.ends_with(".png") || name.ends_with(".ktx2");
        let kept = path
            .file_stem()
            .is_some_and(|s| keep.contains(&*s.to_string_lossy()));
        if name.starts_with(&format!("{prefix}_")) && page && !kept {
            std::fs::remove_file(&path).map_err(|e| format!("remove {name}: {e}"))?;
            // Noted like a write, so the renderer drops the arrays holding it.
            rusty_core::fs::written::note_written(&path.to_string_lossy());
        }
    }
    Ok(())
}

use std::path::PathBuf;
use std::sync::Arc;

use super::file::{encode, occlusion_path, read, Refusal, SUFFIX};
use super::OcclusionData;
use crate::io::Saved;
use crate::Scene;

pub fn save_sidecar(scene: &Scene, scene_path: &str) -> Result<Saved, String> {
    let path = occlusion_path(scene_path);
    let want = scene.occlusion.as_deref().map(encode);
    scene.bake_files.store(&path, want.as_deref(), "Occlusion")
}

pub fn load_sidecar(scene: &mut Scene, scene_path: &str) {
    apply_sidecar(scene, read_sidecar(scene_path));
}

/// Read and decoded from the file alone and put on a scene later, so a load does its work
/// before it touches the live scene.
pub struct OcclusionFile {
    path: PathBuf,
    read: Read,
}

enum Read {
    Missing,
    Unreadable(std::io::Error),
    Refused(Vec<u8>, String),
    OtherVersion(Vec<u8>, String),
    /// The file's bytes, the bytes a save would write (encoded afresh: a file that does not
    /// round-trip never reads as a bake since), and the PVS.
    Decoded(Vec<u8>, Vec<u8>, OcclusionData),
}

pub fn read_sidecar(scene_path: &str) -> OcclusionFile {
    let path = occlusion_path(scene_path);
    let read = match std::fs::read(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Read::Missing,
        Err(e) => Read::Unreadable(e),
        Ok(bytes) => match read(&bytes) {
            Err(Refusal::OtherVersion(why)) => Read::OtherVersion(bytes, why),
            Err(Refusal::Malformed(why)) => Read::Refused(bytes, why),
            Ok(data) => Read::Decoded(bytes, encode(&data), data),
        },
    };
    OcclusionFile { path, read }
}

pub fn apply_sidecar(scene: &mut Scene, file: OcclusionFile) {
    let OcclusionFile { path, read } = file;
    *scene.occlusion = None;
    match read {
        Read::Missing => scene.bake_files.loaded(&path, None, None),
        Read::Unreadable(e) => {
            log::warn!(
                "[Occlusion] could not read {} ({e}): nothing is occlusion-culled, and \
                 the file is kept",
                path.display()
            );
            scene.bake_files.unreadable(&path, &e.to_string())
        }
        Read::Refused(bytes, e) => {
            log::warn!(
                "[Occlusion] {} refused ({e}): nothing is occlusion-culled, and the next \
                 save removes it",
                path.display()
            );
            scene.bake_files.refused(&path, &bytes, &e)
        }
        Read::OtherVersion(bytes, why) => {
            log::warn!("[Occlusion] {} is {why}", path.display());
            scene.bake_files.refused(&path, &bytes, &why)
        }
        Read::Decoded(bytes, held, data) => {
            scene.bake_files.loaded(&path, Some(&bytes), Some(&held));
            *scene.occlusion = Some(Arc::new(data));
        }
    }
}

impl Scene {
    /// The one place a bake's output lands, so its next save writes it, even over a newer
    /// file another process baked.
    pub fn put_baked_occlusion(&mut self, data: Option<OcclusionData>) {
        self.bake_files.baked(SUFFIX);
        *self.occlusion = data.map(Arc::new);
    }

    /// A kept PVS owns its file as a bake's output does: the next save writes it there.
    pub fn keep_baked_occlusion(&mut self) {
        self.bake_files.baked(SUFFIX);
    }
}

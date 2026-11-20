//! The `<scene>.occlusion` file beside the scene (#1714): saved by the bake files'
//! rule, read and decoded (`file`) off the live scene, and put on it, with a
//! warning for a file refused, one another bake version wrote, or one that cannot
//! be read.

use std::path::PathBuf;
use std::sync::Arc;

use super::file::{decode, encode, occlusion_path, other_version, version_of, SUFFIX};
use super::{OcclusionData, OCCLUSION_VERSION};
use crate::io::Saved;
use crate::Scene;

/// Bring the PVS file beside `scene_path` to what `scene` holds, by the bake
/// files' rule (`scene::io::BakeFiles`, #1982): written only when this world baked
/// since it last read or wrote the file, or holds other bytes than it did, and never
/// over a newer file with bytes it only loaded. A file it could not read stays
/// until it bakes; one it refused is rewritten or removed ([`load_sidecar`]).
pub fn save_sidecar(scene: &Scene, scene_path: &str) -> Result<Saved, String> {
    let path = occlusion_path(scene_path);
    let want = scene.occlusion.as_deref().map(encode);
    scene.bake_files.store(&path, want.as_deref(), "Occlusion")
}

/// Load the PVS beside `scene_path` onto `scene`: none when there is no file, and
/// none, with a warning, when the file is refused (the next save then rewrites or
/// removes it, while it is still that file): one another bake version wrote is
/// named so, with what re-bakes it. A file that is there but cannot be read (a permission, a transient
/// failure) also loads none, with a warning, and the next save keeps it.
pub fn load_sidecar(scene: &mut Scene, scene_path: &str) {
    apply_sidecar(scene, read_sidecar(scene_path));
}

/// What a load found at `<scene>.occlusion`, read and decoded from the file alone
/// ([`read_sidecar`]) and put on a scene later ([`apply_sidecar`]), so a load can
/// do the work before it touches the live scene (#2123).
pub struct OcclusionFile {
    path: PathBuf,
    read: Read,
}

enum Read {
    Missing,
    Unreadable(std::io::Error),
    Refused(Vec<u8>, String),
    /// Another bake version's file, refused by its version alone.
    OtherVersion(Vec<u8>, u32),
    /// The file's bytes, what a save would write (encoded afresh: a file that does
    /// not round-trip byte for byte never reads as a bake since), and the PVS.
    Decoded(Vec<u8>, Vec<u8>, OcclusionData),
}

/// Read and decode the PVS beside `scene_path`.
pub fn read_sidecar(scene_path: &str) -> OcclusionFile {
    let path = occlusion_path(scene_path);
    let read = match std::fs::read(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Read::Missing,
        Err(e) => Read::Unreadable(e),
        Ok(bytes) => match decode(&bytes) {
            Err(e) => match version_of(&bytes) {
                Some(v) if v != OCCLUSION_VERSION => Read::OtherVersion(bytes, v),
                _ => Read::Refused(bytes, e),
            },
            Ok(data) => Read::Decoded(bytes, encode(&data), data),
        },
    };
    OcclusionFile { path, read }
}

/// Put what [`read_sidecar`] found on `scene` ([`load_sidecar`] says what each
/// outcome loads).
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
            scene.bake_files.unreadable(&path)
        }
        Read::Refused(bytes, e) => {
            log::warn!(
                "[Occlusion] {} refused ({e}): nothing is occlusion-culled, and the next \
                 save removes it",
                path.display()
            );
            scene.bake_files.refused(&path, &bytes)
        }
        Read::OtherVersion(bytes, version) => {
            log::warn!(
                "[Occlusion] {} is {}",
                path.display(),
                other_version(version)
            );
            scene.bake_files.refused(&path, &bytes)
        }
        Read::Decoded(bytes, held, data) => {
            scene.bake_files.loaded(&path, Some(&bytes), Some(&held));
            *scene.occlusion = Some(Arc::new(data));
        }
    }
}

impl Scene {
    /// Put a bake's PVS on the scene (`None`: the bake kept none): the one place a
    /// bake's output lands, so its next save writes it, even over a newer file
    /// another process baked (#1982).
    pub fn put_baked_occlusion(&mut self, data: Option<OcclusionData>) {
        self.bake_files.baked(SUFFIX);
        *self.occlusion = data.map(Arc::new);
    }

    /// A bake found the PVS the scene holds already current and kept it (#2348):
    /// it owns its file as a bake's output does, so the next save writes it there,
    /// even over a newer file another process wrote (the bytes a re-bake would
    /// write).
    pub fn keep_baked_occlusion(&mut self) {
        self.bake_files.baked(SUFFIX);
    }
}

//! A bake's scratch folder (#2312): each page's files are written here as soon as
//! the page is encoded, so the page stage never holds the pages it finished, and
//! the scene's folder takes them only once the whole bake is done
//! (`write::apply_encoded` links them in). The folder lies beside the scene's
//! lightmap folder, on the same disk so a link is free, named
//! `<scene>.spill-<pid>-<n>.lightmaps`: its name ends like a lightmap folder's,
//! so the texture importer leaves its pages alone
//! (`asset::lightmap_page::is_baked`).
//!
//! It is removed when the bake's pages are dropped: written, cancelled, failed
//! or unwound. A process killed mid-bake cannot remove its own, so the next bake
//! of the scene removes every one whose process is gone, and never reads one: a
//! spill is only ever read by the bake that wrote it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use rusty_asset::lightmap_page::DIR_SUFFIX;

/// The spills opened in this process, numbering their folders.
static OPENED: AtomicU32 = AtomicU32::new(0);

/// One bake's scratch folder; removed on drop.
#[derive(Debug)]
pub struct Spill {
    dir: PathBuf,
    files: u32,
}

/// One file in a [`Spill`]: where it lies and what it holds. Two are equal when
/// their bytes are (their hash and length), wherever they lie.
#[derive(Clone, Debug)]
pub struct Spilled {
    path: PathBuf,
    hash: u64,
    len: u64,
}

impl PartialEq for Spilled {
    fn eq(&self, other: &Self) -> bool {
        (self.hash, self.len) == (other.hash, other.len)
    }
}

/// Every spill is the same as far as the pages go: what they hold is compared
/// through their [`Spilled`] files.
impl PartialEq for Spill {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Spill {
    /// A fresh scratch folder beside `scene_path`, after removing the ones bakes
    /// of the same scene left when their process was killed.
    pub fn open(scene_path: &str) -> Result<Self, String> {
        remove_orphans(scene_path);
        let n = OPENED.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let dir = PathBuf::from(format!("{scene_path}.spill-{pid}-{n}{DIR_SUFFIX}"));
        // A folder of this name is a dead process's that reused this pid.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| format!("lightmap scratch folder: {e}"))?;
        Ok(Self { dir, files: 0 })
    }

    /// Write `bytes` as the next file in the folder.
    pub fn put(&mut self, bytes: &[u8]) -> Result<Spilled, String> {
        let path = self.dir.join(self.files.to_string());
        self.files += 1;
        // lint: allow(asset-write) scratch only its own bake reads back; the page's
        // place in the scene's folder is noted when it is linked there (`write`)
        std::fs::write(&path, bytes).map_err(|e| format!("write {}: {e}", path.display()))?;
        Ok(Spilled {
            path,
            hash: rusty_core::hash::content(bytes),
            len: bytes.len() as u64,
        })
    }

    /// Where it lies.
    #[cfg(test)]
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Spill {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.dir) {
            log::warn!(
                "[LightmapBake] remove the bake's scratch folder {}: {e}",
                self.dir.display()
            );
        }
    }
}

impl Spilled {
    /// Its bytes' hash folded to 32 bits: the page's file name tag.
    pub fn tag(&self) -> u32 {
        (self.hash ^ (self.hash >> 32)) as u32
    }

    /// Its bytes, read back.
    pub fn read(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.path).map_err(|e| format!("read {}: {e}", self.path.display()))
    }

    /// Put it at `to` as well, leaving it in the spill: a hard link on the same
    /// disk, else a copy.
    pub fn link(&self, to: &Path) -> Result<(), String> {
        let _ = std::fs::remove_file(to);
        std::fs::hard_link(&self.path, to)
            .or_else(|_| std::fs::copy(&self.path, to).map(|_| ()))
            .map_err(|e| format!("write {}: {e}", to.display()))
    }
}

/// Remove the spill folders beside `scene_path` whose process is gone.
fn remove_orphans(scene_path: &str) {
    let path = Path::new(scene_path);
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return;
    };
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    let prefix = format!("{}.spill-", name.to_string_lossy());
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(pid) = name
            .strip_prefix(&prefix)
            .and_then(|rest| rest.strip_suffix(DIR_SUFFIX))
            .and_then(|rest| rest.split('-').next()?.parse::<u32>().ok())
        else {
            continue;
        };
        if pid != std::process::id() && !alive(pid) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Whether process `pid` still runs (a signal 0 reaches it, or reaches it but
/// is not ours to send).
fn alive(pid: u32) -> bool {
    unsafe extern "C" {
        fn kill(pid: std::ffi::c_int, sig: std::ffi::c_int) -> std::ffi::c_int;
    }
    let Ok(pid) = std::ffi::c_int::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 sends nothing; it only asks whether `pid` exists.
    let sent = unsafe { kill(pid, 0) };
    // ESRCH (3 on Linux and macOS): no such process.
    sent == 0 || std::io::Error::last_os_error().raw_os_error() != Some(3)
}

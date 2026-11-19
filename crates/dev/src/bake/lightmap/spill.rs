//! A bake's scratch folder (#2312): each page's files are written here as soon as
//! the page is encoded, so the page stage never holds the pages it finished, and
//! the scene's folder takes them only once the whole bake is done
//! (`write::apply_encoded` links them in).
//!
//! **Where:** the project's `cache/` ([`project::CACHE_DIR`], Unity's `Library/`:
//! gitignored and never exported), in [`SPILL_DIR`], one folder a bake named for
//! its scene (`<scene file>-<digest of its path>-<pid>-<n>`). It is on the
//! project's disk, so linking a page into the scene's folder is free; where it is
//! not (a `cache/` on another disk) the page is copied. The project is the
//! nearest folder above the scene that [`project::is_project`]; only a scene in
//! no project at all (a test's, a loose file's) spills into the temp folder, which
//! may be memory (a tmpfs), so its bake's page stage is not bounded.
//!
//! **Removed** when the bake's pages are dropped: written, cancelled, failed or
//! unwound. A process killed mid-bake (a quit, a power cut) cannot remove its own,
//! so each folder has a lock file beside it (`<folder>.lock`) its bake holds for
//! its life: the kernel drops the lock with the process however it ends, and a
//! reboot leaves none held. **Any bake of the project** ([`sweep`], at its start
//! and again when it opens its folder) removes every folder whose lock it can
//! take, whatever scene it was for, and never touches one whose lock is held.
//! A lock counts only while the file at `<folder>.lock` is the one locked
//! ([`take_lock`]): a sweep may delete it between another's open and lock. A
//! spill is only ever read by the bake that wrote it.

use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use rusty_core::project;

/// The folder of bakes' scratch folders, inside the project's `cache/`.
pub const SPILL_DIR: &str = "lightmap-spill";

/// The spills opened in this process, numbering their folders.
static OPENED: AtomicU32 = AtomicU32::new(0);

/// One bake's scratch folder; removed on drop, its lock released.
#[derive(Debug)]
pub struct Spill {
    dir: PathBuf,
    files: u32,
    /// The held lock file, `<dir>.lock`; `None` once let go.
    lock: Option<File>,
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
    /// A fresh scratch folder for a bake of `scene_path`, locked, after
    /// [`sweep`]ing the ones killed bakes left.
    pub fn open(scene_path: &str) -> Result<Self, String> {
        let root = sweep(scene_path);
        let fail =
            |e: std::io::Error| format!("lightmap scratch folder in {}: {e}", root.display());
        std::fs::create_dir_all(&root).map_err(fail)?;
        let pid = std::process::id();
        loop {
            let n = OPENED.fetch_add(1, Ordering::Relaxed);
            let dir = root.join(format!("{}-{pid}-{n}", scene_name(scene_path)));
            let lock = match take_lock(&lock_of(&dir), || {}).map_err(fail)? {
                Taken::Held(lock) => lock,
                // A sweep holds this dead name's lock, or removed the file between
                // its creation and the lock (the lock is then on no file): take
                // the next name.
                Taken::Busy | Taken::Replaced => continue,
            };
            // A folder of this name is a dead process's that had this pid.
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).map_err(fail)?;
            return Ok(Self {
                dir,
                files: 0,
                lock: Some(lock),
            });
        }
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

    /// End as a killed bake does: the lock released, the folder left.
    #[cfg(test)]
    pub fn abandon(mut self) {
        self.lock = None;
    }
}

impl Drop for Spill {
    fn drop(&mut self) {
        let Some(lock) = self.lock.take() else {
            return; // abandoned
        };
        if let Err(e) = std::fs::remove_dir_all(&self.dir) {
            log::warn!(
                "[LightmapBake] remove the bake's scratch folder {}: {e}",
                self.dir.display()
            );
        }
        // Removed while still held, so no sweep takes a lock on a folder in use.
        let _ = std::fs::remove_file(lock_of(&self.dir));
        drop(lock);
    }
}

impl Spilled {
    /// Its bytes' hash folded to 32 bits: the page's file name tag.
    pub fn tag(&self) -> u32 {
        super::write::fold_tag(self.hash)
    }

    /// How many bytes it holds.
    pub fn len(&self) -> u64 {
        self.len
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

/// Remove every scratch folder of `scene_path`'s project whose bake is gone (its
/// lock can be taken), whatever scene it was for; returns the folder they lie in.
pub fn sweep(scene_path: &str) -> PathBuf {
    let root = spill_root(scene_path);
    let Ok(entries) = std::fs::read_dir(&root) else {
        return root; // never spilled
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // A folder's lock, or a lock file whose folder a sweep already removed.
        let (dir, lock) = match path.extension().is_some_and(|e| e == "lock") {
            true => (path.with_extension(""), path),
            false => (path.clone(), lock_of(&path)),
        };
        sweep_one(&dir, &lock, || {});
    }
    root
}

/// Remove the scratch folder `dir` and its lock file `lock` if its bake is gone:
/// only while holding a lock on the very file at `lock`, so a live bake's fresh
/// lock file, made after another sweep removed the one this opened, is never
/// taken for a dead one. `between` runs between the open and the lock (a test's
/// interleaving).
pub(super) fn sweep_one(dir: &Path, lock: &Path, between: impl FnOnce()) {
    if let Ok(Taken::Held(held)) = take_lock(lock, between) {
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_file(lock);
        drop(held);
    }
}

/// What [`take_lock`] got.
#[derive(Debug)]
pub(super) enum Taken {
    /// The lock on the file now at the path.
    Held(File),
    /// Another holds it.
    Busy,
    /// Locked, but the file at the path is no longer the one opened (a sweep
    /// removed it, and maybe another made a new one): the lock guards nothing.
    Replaced,
}

/// Open (creating) the lock file at `path` and try its lock, then check the file
/// still at `path` is the one locked (its device and inode): between the open and
/// the lock, a sweep may lock, delete and release it. `between` runs between the
/// open and the lock: nothing in a bake, a sweep in a test.
pub(super) fn take_lock(path: &Path, between: impl FnOnce()) -> std::io::Result<Taken> {
    use std::os::unix::fs::MetadataExt;
    let file = open_lock(path)?;
    between();
    match file.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return Ok(Taken::Busy),
        Err(TryLockError::Error(e)) => return Err(e),
    }
    let (held, now) = (file.metadata()?, std::fs::metadata(path));
    Ok(match now {
        Ok(now) if (now.dev(), now.ino()) == (held.dev(), held.ino()) => Taken::Held(file),
        _ => Taken::Replaced,
    })
}

/// Where a bake of `scene_path` spills: the [`SPILL_DIR`] of the nearest folder
/// above it that [`project::is_project`] (a `project.rusty`, or an `assets/` folder:
/// a project only ever opened to run has no file), so the spill stays on the
/// project's disk; the temp folder's only for a scene in no project at all, which
/// may be memory (a tmpfs).
fn spill_root(scene_path: &str) -> PathBuf {
    let scene = Path::new(scene_path);
    let folder = scene.parent().filter(|p| !p.as_os_str().is_empty());
    let folder = std::fs::canonicalize(folder.unwrap_or(Path::new(".")));
    let project = folder.ok().and_then(|f| {
        f.ancestors()
            .find(|a| project::is_project(a))
            .map(Path::to_path_buf)
    });
    match project {
        Some(root) => root.join(project::CACHE_DIR).join(SPILL_DIR),
        None => std::env::temp_dir().join("rusty").join(SPILL_DIR),
    }
}

/// A folder name for `scene_path`'s bakes: its file name and a digest of the
/// path, so two scenes of one name in different folders do not share it.
fn scene_name(scene_path: &str) -> String {
    let name = Path::new(scene_path).file_name().unwrap_or_default();
    let digest = rusty_core::hash::content(scene_path.as_bytes()) as u32;
    format!("{}-{digest:08x}", name.to_string_lossy())
}

/// The lock file of the scratch folder `dir`.
fn lock_of(dir: &Path) -> PathBuf {
    let mut name = dir.file_name().unwrap_or_default().to_os_string();
    name.push(".lock");
    dir.with_file_name(name)
}

/// Open (creating) the lock file at `path`, unlocked.
fn open_lock(path: &Path) -> std::io::Result<File> {
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
}

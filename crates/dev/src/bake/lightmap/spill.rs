use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use rusty_core::project;

pub const SPILL_DIR: &str = "lightmap-spill";

static OPENED: AtomicU32 = AtomicU32::new(0);

#[derive(Debug)]
pub struct Spill {
    dir: PathBuf,
    files: u32,
    lock: Option<File>,
}

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

impl PartialEq for Spill {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Spill {
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
                Taken::Busy | Taken::Replaced => continue,
            };
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).map_err(fail)?;
            return Ok(Self {
                dir,
                files: 0,
                lock: Some(lock),
            });
        }
    }

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

    #[cfg(test)]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    #[cfg(test)]
    pub fn abandon(mut self) {
        self.lock = None;
    }
}

impl Drop for Spill {
    fn drop(&mut self) {
        let Some(lock) = self.lock.take() else {
            return;
        };
        if let Err(e) = std::fs::remove_dir_all(&self.dir) {
            log::warn!(
                "[LightmapBake] remove the bake's scratch folder {}: {e}",
                self.dir.display()
            );
        }
        let _ = std::fs::remove_file(lock_of(&self.dir));
        drop(lock);
    }
}

impl Spilled {
    pub fn tag(&self) -> u32 {
        super::write::fold_tag(self.hash)
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn read(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.path).map_err(|e| format!("read {}: {e}", self.path.display()))
    }

    pub fn link(&self, to: &Path) -> Result<(), String> {
        let _ = std::fs::remove_file(to);
        std::fs::hard_link(&self.path, to)
            .or_else(|_| std::fs::copy(&self.path, to).map(|_| ()))
            .map_err(|e| format!("write {}: {e}", to.display()))
    }
}

pub fn sweep(scene_path: &str) -> PathBuf {
    let root = spill_root(scene_path);
    let Ok(entries) = std::fs::read_dir(&root) else {
        return root;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let (dir, lock) = match path.extension().is_some_and(|e| e == "lock") {
            true => (path.with_extension(""), path),
            false => (path.clone(), lock_of(&path)),
        };
        sweep_one(&dir, &lock, || {});
    }
    root
}

pub(super) fn sweep_one(dir: &Path, lock: &Path, between: impl FnOnce()) {
    if let Ok(Taken::Held(held)) = take_lock(lock, between) {
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_file(lock);
        drop(held);
    }
}

#[derive(Debug)]
pub(super) enum Taken {
    Held(File),
    Busy,
    Replaced,
}

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

fn scene_name(scene_path: &str) -> String {
    let name = Path::new(scene_path).file_name().unwrap_or_default();
    let digest = rusty_core::hash::content(scene_path.as_bytes()) as u32;
    format!("{}-{digest:08x}", name.to_string_lossy())
}

fn lock_of(dir: &Path) -> PathBuf {
    let mut name = dir.file_name().unwrap_or_default().to_os_string();
    name.push(".lock");
    dir.with_file_name(name)
}

fn open_lock(path: &Path) -> std::io::Result<File> {
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
}

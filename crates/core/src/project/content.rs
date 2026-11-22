use std::path::{Path, PathBuf};

pub const ENGINE_PREFIX: &str = "engine/";

pub fn engine_relative(path: &str) -> Option<String> {
    let stored = crate::paths::stored(path);
    let parts: Vec<&str> = stored.split('/').filter(|p| !p.is_empty()).collect();
    let relative = parts.join("/");
    // A rooted path keeps its `/`, so it is never `engine/…` and never joins as `/`.
    let normal = if stored.starts_with('/') {
        format!("/{relative}")
    } else {
        relative
    };
    normal.strip_prefix(ENGINE_PREFIX).map(str::to_string)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentRoots {
    pub project: PathBuf,
    pub engine: PathBuf,
}

impl ContentRoots {
    pub fn new(project: &Path) -> Self {
        Self::with_engine(project, super::engine_dir())
    }

    pub fn with_engine(project: &Path, engine: &Path) -> Self {
        Self {
            project: project.to_path_buf(),
            engine: engine.to_path_buf(),
        }
    }

    pub fn base(&self, path: &str) -> &Path {
        if engine_relative(path).is_some() {
            &self.engine
        } else {
            &self.project
        }
    }

    pub fn file(&self, path: &str) -> PathBuf {
        match engine_relative(path) {
            Some(relative) => self.engine.join(relative),
            None => self.project.join(path),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_engine_path_reads_from_the_engine_and_any_other_from_the_project() {
        let roots = ContentRoots::with_engine(Path::new("/game"), Path::new("/install/engine"));
        let engine = roots.file("engine/textures/library/rust.graph.json");
        assert_eq!(
            engine,
            Path::new("/install/engine/textures/library/rust.graph.json")
        );
        assert_eq!(roots.base("engine/a.png"), Path::new("/install/engine"));
        assert_eq!(roots.file("assets/a.png"), Path::new("/game/assets/a.png"));
        assert_eq!(
            roots.file("engines/a.png"),
            Path::new("/game/engines/a.png")
        );
        assert_eq!(
            roots.file("assets/engine/a.png"),
            Path::new("/game/assets/engine/a.png")
        );
    }

    #[test]
    fn an_engine_path_is_matched_as_stored_and_never_joins_the_filesystem_root() {
        let roots = ContentRoots::with_engine(Path::new("/game"), Path::new("/install/engine"));
        let want = Path::new("/install/engine/textures/a.png");
        assert_eq!(roots.file("engine//textures/a.png"), want);
        assert_eq!(roots.file("./engine/textures/a.png"), want);
        assert_eq!(roots.file(" engine/textures//a.png"), want);
        assert_eq!(
            roots.base("./engine/textures/a.png"),
            Path::new("/install/engine")
        );
        assert_eq!(engine_relative("/engine/a.png"), None);
        assert_eq!(engine_relative("//engine/a.png"), None);
    }

    #[test]
    fn an_empty_project_root_leaves_a_project_path_as_written() {
        let roots = ContentRoots::with_engine(Path::new(""), Path::new("/e"));
        assert_eq!(roots.file("assets/dirt.png"), Path::new("assets/dirt.png"));
        assert_eq!(roots.file("engine/dirt.png"), Path::new("/e/dirt.png"));
    }

    #[test]
    fn a_shipped_layout_reads_an_engine_graph_from_the_engine_beside_the_player() {
        let ship = rusty_test_support::temp::dir().join("rusty_content_ship_2185");
        let engine = ship.join("engine");
        std::fs::create_dir_all(engine.join("shaders")).unwrap();
        std::fs::create_dir_all(engine.join("textures/library")).unwrap();
        std::fs::write(engine.join("shaders/common.wgsl"), "").unwrap();
        std::fs::write(engine.join("textures/library/rust.graph.json"), "{}").unwrap();
        let found = super::super::locate::find_engine_dir(&ship).unwrap();
        let roots = ContentRoots::with_engine(&ship.join("project"), &found);
        let file = roots.file("engine/textures/library/rust.graph.json");
        assert!(file.is_file(), "{}", file.display());
    }
}

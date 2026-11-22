use rusty_scene::lighting::io::{read_lighting_sidecar, LightingFile};
use rusty_scene::nav_settings::{read_navmesh, NavmeshFile};
use rusty_scene::serialize::{RehydratedScene, SceneData};

pub struct SceneSidecars {
    pub(super) navmesh: NavmeshFile,
    pub(super) lighting: LightingFile,
}

impl SceneSidecars {
    pub fn read(path: &str) -> Self {
        Self {
            navmesh: read_navmesh(path),
            lighting: read_lighting_sidecar(path),
        }
    }

    pub fn has_navmesh(&self) -> bool {
        self.navmesh.has_bytes()
    }
}

pub struct PreparedScene {
    pub(super) path: String,
    pub(super) document: RehydratedScene,
    pub(super) sidecars: SceneSidecars,
    pub(super) baked_navmesh: Option<Vec<u8>>,
}

impl PreparedScene {
    pub fn new(path: String, document: RehydratedScene, sidecars: SceneSidecars) -> Self {
        Self {
            path,
            document,
            sidecars,
            baked_navmesh: None,
        }
    }

    pub fn from_data(data: SceneData, path: &str) -> Self {
        let document = RehydratedScene::new(data);
        Self::new(path.to_string(), document, SceneSidecars::read(path))
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn data(&self) -> &SceneData {
        self.document.data()
    }

    pub fn set_baked_navmesh(&mut self, bytes: Vec<u8>) {
        self.baked_navmesh = Some(bytes);
    }

    pub fn take_baked_navmesh(&mut self) -> Option<Vec<u8>> {
        self.baked_navmesh.take()
    }
}

pub fn staged(document: RehydratedScene) -> rusty_scene::Scene {
    let mut scene = rusty_scene::Scene::new();
    crate::serialize::apply_rehydrated(&mut scene, document);
    scene
}

//! Unity's `GameObject.scene`, as a membership: the World stays one. Only roots are tagged;
//! a child is its root's scene, an untagged root the active one's (so a spawn joins it), and a
//! change of active scene first tags every untagged root with the old one.

use std::collections::BTreeMap;

use glam::Vec3;

use crate::Scene;

pub const DONT_DESTROY_ON_LOAD: &str = "DontDestroyOnLoad";

#[derive(Clone, Debug, PartialEq)]
pub struct SceneSettings {
    pub skybox_path: String,
    pub ambient_color: Vec3,
    pub ambient_intensity: f32,
    pub ambient: crate::AmbientSettings,
    pub nav_settings: crate::nav_settings::NavMeshSettings,
    pub fog: crate::FogSettings,
    pub sky: crate::SkySettings,
    pub lighting_settings: crate::lighting::LightingSettings,
}

#[derive(Clone, Debug)]
struct Loaded {
    handle: u32,
    path: Option<String>,
    settings: Option<SceneSettings>,
}

/// Empty: the World's one scene. Otherwise in load order, the `"Single"` one first.
#[derive(Clone, Debug, Default)]
pub struct LoadedScenes {
    scenes: Vec<Loaded>,
    active: u32,
    next: u32,
    roots: BTreeMap<u32, u32>,
    unloads: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Member {
    DontDestroyOnLoad,
    Scene(Option<u32>),
}

impl LoadedScenes {
    pub fn paths(&self, active: Option<&str>) -> Vec<Option<String>> {
        match self.scenes.is_empty() {
            true => vec![active.map(str::to_string)],
            false => self.scenes.iter().map(|s| s.path.clone()).collect(),
        }
    }

    pub fn path_of(&self, handle: u32) -> Option<&str> {
        let scene = self.scenes.iter().find(|s| s.handle == handle)?;
        scene.path.as_deref()
    }

    pub fn handle_of(&self, path: &str) -> Option<u32> {
        let found = self.scenes.iter().find(|s| s.path.as_deref() == Some(path));
        found.map(|s| s.handle)
    }

    /// When it moves, the renderer releases what an unloaded scene used.
    pub fn unloads(&self) -> u64 {
        self.unloads
    }

    /// Test hook: the inventory's mark.
    #[doc(hidden)]
    pub fn tag(&mut self, root: u32, handle: u32) {
        self.roots.insert(root, handle);
    }

    #[doc(hidden)]
    pub fn tag_of(&self, root: u32) -> Option<u32> {
        self.roots.get(&root).copied()
    }

    fn entry(&mut self, handle: u32) -> Option<&mut Loaded> {
        self.scenes.iter_mut().find(|s| s.handle == handle)
    }
}

impl Scene {
    pub fn member_of(&self, id: u32) -> Option<Member> {
        if !self.world.contains(id) {
            return None;
        }
        let line = || std::iter::once(id).chain(self.world.ancestors(id));
        if line().any(|a| self.persistent.contains(&a)) {
            return Some(Member::DontDestroyOnLoad);
        }
        let root = line().last().unwrap_or(id);
        let loaded = &self.loaded;
        let handle = match loaded.scenes.is_empty() {
            true => None,
            false => Some(loaded.roots.get(&root).copied().unwrap_or(loaded.active)),
        };
        Some(Member::Scene(handle))
    }

    pub fn member_path(&self, id: u32, active: Option<&str>) -> Option<String> {
        match self.member_of(id)? {
            Member::DontDestroyOnLoad => Some(DONT_DESTROY_ON_LOAD.to_string()),
            Member::Scene(None) => active.map(str::to_string),
            Member::Scene(Some(handle)) => self.loaded.path_of(handle).map(str::to_string),
        }
    }

    pub fn reset_loaded(&mut self) {
        self.loaded = LoadedScenes {
            unloads: self.loaded.unloads,
            ..Default::default()
        };
    }

    /// `base` names the World's scene on its first additive load.
    pub fn join_loaded(
        &mut self,
        base: Option<String>,
        path: String,
        settings: SceneSettings,
        roots: &[u32],
    ) -> u32 {
        let loaded = &mut self.loaded;
        if loaded.scenes.is_empty() {
            loaded.scenes.push(Loaded {
                handle: 0,
                path: base,
                settings: None,
            });
            loaded.active = 0;
            loaded.next = 1;
        }
        let handle = loaded.next;
        loaded.next += 1;
        let settings = Some(settings);
        let path = Some(path);
        loaded.scenes.push(Loaded {
            handle,
            path,
            settings,
        });
        for &root in roots {
            loaded.roots.insert(root, handle);
        }
        handle
    }

    pub fn active_handle(&self) -> Option<u32> {
        (!self.loaded.scenes.is_empty()).then_some(self.loaded.active)
    }

    /// Unity's `SetActiveScene`.
    pub fn set_active_scene(&mut self, handle: u32) -> Option<String> {
        self.loaded.entry(handle)?;
        if handle != self.loaded.active {
            self.freeze_roots();
            let held = Some(self.settings());
            let old = self.loaded.active;
            if let Some(entry) = self.loaded.entry(old) {
                entry.settings = held;
            }
            let next = self.loaded.entry(handle).and_then(|e| e.settings.take());
            if let Some(settings) = next {
                self.put_settings(settings);
            }
            self.loaded.active = handle;
        }
        Some(self.loaded.path_of(handle).unwrap_or_default().to_string())
    }

    /// Queues the members' destroys; refuses the last scene.
    pub fn unload_scene(&mut self, handle: u32) -> Result<Option<String>, String> {
        if self.loaded.scenes.len() < 2 {
            return Err("the last loaded scene cannot be unloaded".to_string());
        }
        if self.loaded.entry(handle).is_none() {
            return Err("that scene is not loaded".to_string());
        }
        self.freeze_roots();
        let members: Vec<u32> = self.roots_where(|m| m == Member::Scene(Some(handle)));
        for &id in &members {
            self.request_destroy(id);
            self.loaded.roots.remove(&id);
        }
        if handle == self.loaded.active {
            let next = self.loaded.scenes.iter().find(|s| s.handle != handle);
            let next = next.map(|s| s.handle).unwrap_or_default();
            self.set_active_scene(next);
        }
        self.loaded.scenes.retain(|s| s.handle != handle);
        self.loaded.unloads += 1;
        Ok(self.loaded.path_of(self.loaded.active).map(str::to_string))
    }

    fn roots_where(&self, keep: impl Fn(Member) -> bool) -> Vec<u32> {
        let roots = self.world.ids().iter().copied();
        let roots = roots.filter(|&id| self.world.parent_id(id).is_none());
        let kept = roots.filter(|&id| self.member_of(id).is_some_and(&keep));
        kept.collect()
    }

    fn freeze_roots(&mut self) {
        let world = &self.world;
        self.loaded.roots.retain(|&id, _| world.contains(id));
        let active = self.loaded.active;
        let untagged = self.roots_where(|m| m == Member::Scene(Some(active)));
        for id in untagged {
            self.loaded.roots.entry(id).or_insert(active);
        }
    }

    pub fn settings(&self) -> SceneSettings {
        SceneSettings {
            skybox_path: self.skybox_path.clone(),
            ambient_color: self.ambient_color,
            ambient_intensity: self.ambient_intensity,
            ambient: self.ambient,
            nav_settings: self.nav_settings.clone(),
            fog: self.fog,
            sky: self.sky,
            lighting_settings: self.lighting_settings,
        }
    }

    fn put_settings(&mut self, settings: SceneSettings) {
        self.skybox_path = settings.skybox_path;
        self.ambient_color = settings.ambient_color;
        self.ambient_intensity = settings.ambient_intensity;
        self.ambient = settings.ambient;
        self.nav_settings = settings.nav_settings;
        self.fog = settings.fog;
        self.sky = settings.sky;
        self.lighting_settings = settings.lighting_settings;
    }
}

#[cfg(test)]
#[path = "membership_tests.rs"]
mod tests;

use mlua::{Lua, RegistryKey, Table};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use crate::api::ApiScopedCtx;
use crate::core::input::InputState;
use crate::core::storage::Storage;
use crate::core::video::VideoSettings;
use crate::navigation::NavigationGraph;
use crate::render::postfx::QualityPreset;
use crate::render::Camera;
use crate::scene::Scene;
use crate::time::Time;

use super::console::ConsoleLogs;

pub struct ScriptManager {
    pub(super) lua: Option<Lua>,
    /// One lifecycle table per attached script, keyed by `(entity_id,
    /// script_index)`. An entity can carry many scripts (#83); each keeps its own
    /// state, and the index is the slot in the entity's `scripts` vec so two
    /// scripts on one entity never collide.
    pub(super) entity_scripts: HashMap<(u32, usize), RegistryKey>,
    pub(super) scene: Rc<RefCell<Scene>>,
    pub(super) input: Rc<RefCell<InputState>>,
    pub(super) nav: Rc<RefCell<NavigationGraph>>,
    pub(super) console: Rc<RefCell<ConsoleLogs>>,
    pub(super) camera: Rc<RefCell<Camera>>,
    pub(super) time: Rc<RefCell<Time>>,
    /// The persistent key-value store, shared with the app so it can flush at
    /// boundaries. Injected by `Resources` via [`ScriptManager::set_storage`];
    /// defaults to an empty, pathless store (harness/tests never touch disk).
    pub(super) storage: Rc<RefCell<Storage>>,
    /// Global post-FX scalability tier (`Graphics.Get/SetQuality`). Shared with
    /// the platform layer, which reads it each frame and hands it to
    /// `renderer.set_quality`; defaults to `Medium` (harness/tests need no GPU).
    pub(super) quality: Rc<RefCell<QualityPreset>>,
    /// Runtime video settings (`Video.*`: resolution / vsync / fullscreen). Shared
    /// with the platform layer, which reads it each frame and reconfigures the
    /// surface + window; defaults to the windowed boot settings (harness needs no
    /// window).
    pub(super) video: Rc<RefCell<VideoSettings>>,
    /// Live rapier physics world shared with `GameWorld` — used by
    /// `Physics.Raycast`/`Shoot`. `None` until Play builds the world.
    pub(super) physics: Rc<RefCell<Option<crate::physics::PhysicsWorld>>>,
    /// The scene file the editor/session is currently editing — the write-back
    /// target for `Scene.Save()` (no-path). Shared with the platform layer (the
    /// editor keeps `EditorUi.current_scene_path` synced into it), so a scripted
    /// save and an editor save hit the same file. `None` until a scene is loaded
    /// or the path is set; an argument-less `Scene.Save()` then errors cleanly.
    pub(super) scene_path: Rc<RefCell<Option<String>>>,
}

impl ScriptManager {
    pub fn new(
        scene: Rc<RefCell<Scene>>,
        input: Rc<RefCell<InputState>>,
        nav: Rc<RefCell<NavigationGraph>>,
        console: Rc<RefCell<ConsoleLogs>>,
        camera: Rc<RefCell<Camera>>,
        time: Rc<RefCell<Time>>,
    ) -> Self {
        Self {
            lua: None,
            entity_scripts: HashMap::new(),
            scene,
            input,
            nav,
            console,
            camera,
            time,
            storage: Rc::new(RefCell::new(Storage::new())),
            quality: Rc::new(RefCell::new(QualityPreset::default())),
            video: Rc::new(RefCell::new(VideoSettings::default())),
            physics: Rc::new(RefCell::new(None)),
            scene_path: Rc::new(RefCell::new(None)),
        }
    }

    /// Inject the shared scene-path cell — the `Scene.Save()` write-back target.
    /// The platform layer keeps this in sync with `EditorUi.current_scene_path`,
    /// and the headless session sets it from the loaded boot scene, so a scripted
    /// save writes back to the same file the editor would.
    pub fn set_scene_path_cell(&mut self, scene_path: Rc<RefCell<Option<String>>>) {
        self.scene_path = scene_path;
    }

    /// Handle to the shared scene-path cell, so the platform layer can read/sync
    /// the current scene file with the editor's `current_scene_path`.
    pub fn scene_path_cell(&self) -> Rc<RefCell<Option<String>>> {
        Rc::clone(&self.scene_path)
    }

    /// Inject the shared persistent store. `Resources` calls this so the script
    /// runtime, the console REPL and the app all read/write the same `Storage`.
    pub fn set_storage(&mut self, storage: Rc<RefCell<Storage>>) {
        self.storage = storage;
    }

    /// Inject the shared quality-preset cell. The platform layer keeps this cell
    /// in sync with the renderer's tier, so `Graphics.SetQuality` from a script
    /// reaches `renderer.set_quality` (which guards the bloom-buffer realloc).
    pub fn set_quality_cell(&mut self, quality: Rc<RefCell<QualityPreset>>) {
        self.quality = quality;
    }

    /// Handle to the shared quality-preset cell, so the platform layer can read a
    /// script-driven tier change and apply it to the renderer.
    pub fn quality_cell(&self) -> Rc<RefCell<QualityPreset>> {
        Rc::clone(&self.quality)
    }

    /// Inject the shared video-settings cell. The platform layer keeps this in sync
    /// with the live surface/window, so `Video.*` writes from a script reach the
    /// wgpu surface (resolution / present mode) and the winit window (fullscreen).
    pub fn set_video_cell(&mut self, video: Rc<RefCell<VideoSettings>>) {
        self.video = video;
    }

    /// Handle to the shared video-settings cell, so the platform layer can read a
    /// script-driven resolution / vsync / fullscreen change and apply it.
    pub fn video_cell(&self) -> Rc<RefCell<VideoSettings>> {
        Rc::clone(&self.video)
    }

    /// Initializes a fresh Lua environment. API namespaces are NOT registered
    /// here; they are registered per-evaluation inside `lua.scope(...)` in
    /// `lifecycle.rs`, so closures capture `&RefCell<T>` references (no
    /// `Rc::clone` in the `api/` tree).
    ///
    /// `physics` is the live rapier world shared with `GameWorld`; stored here
    /// so `lifecycle.rs` can dereference it when building `ApiScopedCtx`.
    pub fn init_runtime(
        &mut self,
        physics: &Rc<RefCell<Option<crate::physics::PhysicsWorld>>>,
    ) -> Result<(), String> {
        self.physics = Rc::clone(physics);

        let lua = Lua::new();

        // Override print to write to our console panel. This stays as a
        // `lua.create_function` (static closure) because `print` is in the
        // scripting layer, not the api/ tree — the Rc borrow here is fine.
        let console_clone = Rc::clone(&self.console);
        let print_fn = lua
            .create_function(move |_, msg: String| {
                console_clone.borrow_mut().info(msg);
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        lua.globals()
            .set("print", print_fn)
            .map_err(|e| e.to_string())?;

        self.lua = Some(lua);
        self.entity_scripts.clear();

        Ok(())
    }

    /// Build the scoped context from this manager's resource cells.
    ///
    /// The returned struct borrows through the `Rc` smart pointers, tying
    /// references to the caller's lifetime. Used inside `lua.scope(...)` blocks in
    /// `lifecycle.rs` so the borrow can't outlive the scope.
    pub(super) fn make_ctx(&self) -> ApiScopedCtx<'_> {
        ApiScopedCtx {
            scene: &self.scene,
            input: &self.input,
            nav: &self.nav,
            camera: &self.camera,
            time: &self.time,
            physics: &self.physics,
            console: &self.console,
            storage: &self.storage,
            quality: &self.quality,
            video: &self.video,
            scene_path: &self.scene_path,
        }
    }

    /// Loads and runs one of an entity's scripts, then registers its lifecycle
    /// methods. `script_index` is the slot in the entity's `scripts` vec, so an
    /// entity's many scripts (#83) each get their own keyed lifecycle table.
    pub fn load_entity_script(
        &mut self,
        entity_id: u32,
        script_index: usize,
        script_path: &str,
        field_values: &std::collections::BTreeMap<String, crate::components::ScriptFieldValue>,
    ) -> Result<(), String> {
        let lua = self.lua.as_ref().ok_or("Lua runtime not initialized")?;

        if !Path::new(script_path).exists() {
            return Err(format!("Script file not found: {}", script_path));
        }

        let script_code = std::fs::read_to_string(script_path)
            .map_err(|e| format!("Failed to read script: {}", e))?;

        // Load the script chunk
        let chunk = lua.load(&script_code);
        let table: Table = chunk
            .eval()
            .map_err(|e| format!("Syntax error compiling {}: {}", script_path, e))?;

        // Merge the inspector-set field values (#84) over the schema defaults onto
        // the lifecycle table, so `self.<field>` inside the script reads the
        // configured value. Pure data assignment — determinism-clean.
        super::schema::apply_field_values(&table, &script_code, field_values)
            .map_err(|e| format!("Failed to apply script fields for {}: {}", script_path, e))?;

        // Cache the returned lifecycle table in the Lua registry
        let reg_key = lua
            .create_registry_value(table)
            .map_err(|e| format!("Failed to register script table: {}", e))?;

        self.entity_scripts
            .insert((entity_id, script_index), reg_key);

        Ok(())
    }

    /// Stops and clears the script environment
    pub fn shutdown(&mut self) {
        self.entity_scripts.clear();
        self.lua = None;
    }

    /// Run a Lua chunk against the live runtime. Test-only entry point for the
    /// issue-#5 API coverage; the windowed/headless paths drive scripts through
    /// the lifecycle callbacks instead.
    #[cfg(test)]
    pub(super) fn exec(&self, code: &str) -> Result<(), String> {
        use crate::api;
        let lua = self.lua.as_ref().ok_or("runtime not initialized")?;
        let ctx = self.make_ctx();
        lua.scope(|scope| {
            api::register(lua, scope, &ctx).map_err(mlua::Error::RuntimeError)?;
            lua.load(code).exec()
        })
        .map_err(|e| e.to_string())
    }

    /// Whether the live runtime exists yet (only during play / a loaded scenario).
    pub fn is_live(&self) -> bool {
        self.lua.is_some()
    }
}

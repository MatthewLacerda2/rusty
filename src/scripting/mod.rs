use mlua::{Lua, RegistryKey, Table};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use crate::api::{self, ApiCtx};
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::render::Camera;
use crate::scene::Scene;
use crate::time::Time;

pub struct ConsoleLogs {
    pub messages: Vec<(String, LogLevel)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

impl Default for ConsoleLogs {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsoleLogs {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    pub fn info(&mut self, msg: String) {
        self.add_log(msg, LogLevel::Info);
    }

    pub fn warn(&mut self, msg: String) {
        self.add_log(msg, LogLevel::Warning);
    }

    pub fn error(&mut self, msg: String) {
        self.add_log(msg, LogLevel::Error);
    }

    fn add_log(&mut self, msg: String, level: LogLevel) {
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
        self.messages.push((msg, level));
    }
}

pub struct ScriptManager {
    lua: Option<Lua>,
    entity_scripts: HashMap<u32, RegistryKey>,
    scene: Rc<RefCell<Scene>>,
    input: Rc<RefCell<InputState>>,
    nav: Rc<RefCell<NavigationGraph>>,
    console: Rc<RefCell<ConsoleLogs>>,
    camera: Rc<RefCell<Camera>>,
    time: Rc<RefCell<Time>>,
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
        }
    }

    /// Initializes a fresh Lua environment and registers all required namespaces.
    /// `physics` is the live rapier world shared with `GameWorld`; the
    /// `Physics.Raycast`/`Shoot` bindings cast against it so scripts and the
    /// engine hitscan agree. It is `None` until Play builds the world.
    pub fn init_runtime(
        &mut self,
        physics: &Rc<RefCell<Option<crate::physics::PhysicsWorld>>>,
    ) -> Result<(), String> {
        let lua = Lua::new();

        // 1. Override print to write to our console panel
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

        // 2. Register the whole stable API surface. Every namespace
        //    (`Transform`, `Material`, `Animator`, `Input`, `Scene`, `Navigation`,
        //    `NavMeshAgent`, `Physics`, `Health`, `Time`, `Camera`, and the
        //    dev-only `Debug`) is owned by the `api/` tree — the single surface
        //    shared by gameplay scripts, the console REPL and bot-players. The
        //    live `physics` handle rides along so `Physics.Raycast`/`Shoot` reach
        //    the same rapier world the engine hitscan uses (#31).
        let ctx = ApiCtx {
            scene: Rc::clone(&self.scene),
            input: Rc::clone(&self.input),
            nav: Rc::clone(&self.nav),
            camera: Rc::clone(&self.camera),
            time: Rc::clone(&self.time),
            physics: Rc::clone(physics),
            console: Rc::clone(&self.console),
        };
        api::register(&lua, &ctx)?;

        self.lua = Some(lua);
        self.entity_scripts.clear();

        Ok(())
    }

    /// Loads and runs a script file, then registers its lifecycle methods
    pub fn load_entity_script(&mut self, entity_id: u32, script_path: &str) -> Result<(), String> {
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

        // Cache the returned lifecycle table in the Lua registry
        let reg_key = lua
            .create_registry_value(table)
            .map_err(|e| format!("Failed to register script table: {}", e))?;

        self.entity_scripts.insert(entity_id, reg_key);

        Ok(())
    }

    /// Invokes the Start function on all loaded entity scripts
    pub fn start_scripts(&mut self) {
        let lua = match &self.lua {
            Some(l) => l,
            None => return,
        };

        // Sorted so script Start order is deterministic (HashMap iteration order
        // varies per run); replays must be byte-identical.
        let mut ids: Vec<u32> = self.entity_scripts.keys().copied().collect();
        ids.sort_unstable();

        for id in ids {
            if let Some(key) = self.entity_scripts.get(&id) {
                if let Ok(table) = lua.registry_value::<Table>(key) {
                    if let Ok(start_fn) = table.get::<_, mlua::Function>("Start") {
                        if let Err(e) = start_fn.call::<_, ()>(id) {
                            self.console
                                .borrow_mut()
                                .error(format!("[Lua Error] Start on entity {} failed: {}", id, e));
                        }
                    }
                }
            }
        }
    }

    /// Invokes the Update function on all loaded entity scripts
    pub fn update_scripts(&mut self, delta_time: f32) {
        let lua = match &self.lua {
            Some(l) => l,
            None => return,
        };

        // Filter active entities in the scene that have scripts
        let scene = self.scene.borrow();
        let mut ids: Vec<u32> = self
            .entity_scripts
            .keys()
            .copied()
            .filter(|&id| {
                if let Some(e) = scene.get_entity(id) {
                    e.active
                } else {
                    false
                }
            })
            .collect();
        drop(scene);
        // Sorted so per-frame Update order is deterministic (HashMap iteration
        // order varies per run). Gameplay now happens inside scripts — e.g. the
        // weapon's Physics.Shoot vs. the enemy's animation Update race in the
        // kill frame — so a stable order is what keeps replays byte-identical.
        ids.sort_unstable();

        for id in ids {
            if let Some(key) = self.entity_scripts.get(&id) {
                if let Ok(table) = lua.registry_value::<Table>(key) {
                    if let Ok(update_fn) = table.get::<_, mlua::Function>("Update") {
                        if let Err(e) = update_fn.call::<_, ()>((id, delta_time)) {
                            self.console.borrow_mut().error(format!(
                                "[Lua Error] Update on entity {} failed: {}",
                                id, e
                            ));
                        }
                    }
                }
            }
        }
    }

    /// Invokes the OnTrigger callback on scripts of entities involved in a trigger overlap
    pub fn dispatch_trigger_events(&mut self, events: Vec<(u32, u32)>) {
        let lua = match &self.lua {
            Some(l) => l,
            None => return,
        };

        for (id_a, id_b) in events {
            if let Some(key_a) = self.entity_scripts.get(&id_a) {
                if let Ok(table) = lua.registry_value::<Table>(key_a) {
                    if let Ok(trigger_fn) = table.get::<_, mlua::Function>("OnTrigger") {
                        if let Err(e) = trigger_fn.call::<_, ()>((id_a, id_b)) {
                            self.console.borrow_mut().error(format!(
                                "[Lua Error] OnTrigger on entity {} failed: {}",
                                id_a, e
                            ));
                        }
                    }
                }
            }
            if let Some(key_b) = self.entity_scripts.get(&id_b) {
                if let Ok(table) = lua.registry_value::<Table>(key_b) {
                    if let Ok(trigger_fn) = table.get::<_, mlua::Function>("OnTrigger") {
                        if let Err(e) = trigger_fn.call::<_, ()>((id_b, id_a)) {
                            self.console.borrow_mut().error(format!(
                                "[Lua Error] OnTrigger on entity {} failed: {}",
                                id_b, e
                            ));
                        }
                    }
                }
            }
        }
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
    fn exec(&self, code: &str) -> Result<(), String> {
        let lua = self.lua.as_ref().ok_or("runtime not initialized")?;
        lua.load(code).exec().map_err(|e| e.to_string())
    }

    /// Whether the live runtime exists yet (only during play / a loaded scenario).
    pub fn is_live(&self) -> bool {
        self.lua.is_some()
    }

    /// Evaluate ONE line of Lua against the LIVE runtime and return the result as
    /// a display string. This is the single evaluator behind both the in-editor
    /// console input line and the headless harness, so the two can never drift.
    ///
    /// A REPL line is first tried as an expression (`return <line>`) so values are
    /// echoed back; if that fails to compile it is run as a statement (assignments,
    /// `print(...)`, control flow). Multiple returned values are comma-joined.
    pub fn eval(&self, line: &str) -> Result<String, String> {
        let lua = self
            .lua
            .as_ref()
            .ok_or_else(|| "runtime not initialized (enter Play to evaluate)".to_string())?;

        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(String::new());
        }

        // Try as an expression first so REPL lines echo their value.
        let values = match lua
            .load(format!("return {}", trimmed))
            .eval::<mlua::MultiValue>()
        {
            Ok(v) => v,
            Err(_) => lua
                .load(trimmed)
                .eval::<mlua::MultiValue>()
                .map_err(|e| e.to_string())?,
        };

        let rendered = values
            .iter()
            .map(value_to_string)
            .collect::<Vec<_>>()
            .join(", ");
        Ok(rendered)
    }
}

/// Render a single Lua value for the REPL echo. Mirrors Lua's `tostring`/`print`
/// for the common cases (nil/bool/number/string) and falls back to a typed tag.
fn value_to_string(value: &mlua::Value) -> String {
    match value {
        mlua::Value::Nil => "nil".to_string(),
        mlua::Value::Boolean(b) => b.to_string(),
        mlua::Value::Integer(i) => i.to_string(),
        mlua::Value::Number(n) => n.to_string(),
        mlua::Value::String(s) => s.to_str().map(|s| s.to_string()).unwrap_or_default(),
        mlua::Value::Table(_) => "<table>".to_string(),
        mlua::Value::Function(_) => "<function>".to_string(),
        other => format!("<{}>", other.type_name()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::HealthComponent;
    use crate::render::Camera;
    use crate::time::Time;
    use glam::Vec3;

    fn manager() -> (ScriptManager, Rc<RefCell<Scene>>, Rc<RefCell<Camera>>) {
        let mut raw = Scene::new();
        let id = raw.add_entity("Target".to_string());
        if let Some(mut e) = raw.get_entity_mut(id) {
            e.health = Some(HealthComponent {
                current_health: 100.0,
                max_health: 100.0,
                is_dead: false,
            });
        }
        let scene = Rc::new(RefCell::new(raw));
        let input = Rc::new(RefCell::new(InputState::new()));
        let nav = Rc::new(RefCell::new(NavigationGraph::new(
            -10.0, 10.0, -10.0, 10.0, 1.0,
        )));
        let console = Rc::new(RefCell::new(ConsoleLogs::new()));
        let camera = Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0)));
        let time = Rc::new(RefCell::new(Time::new()));
        time.borrow_mut().advance(0.25);
        let mut m = ScriptManager::new(
            Rc::clone(&scene),
            input,
            nav,
            console,
            Rc::clone(&camera),
            time,
        );
        // No live physics world in these unit tests, so Physics.Raycast/Shoot miss.
        let physics = Rc::new(RefCell::new(None));
        m.init_runtime(&physics).expect("runtime inits");
        (m, scene, camera)
    }

    #[test]
    fn health_get_damage_heal_roundtrip() {
        let (m, scene, _cam) = manager();
        m.exec("Health.Damage(1, 30)").unwrap();
        m.exec("Health.Heal(1, 5)").unwrap();
        let hp = scene
            .borrow()
            .get_entity(1)
            .and_then(|e| e.health.as_ref().map(|h| h.current_health));
        assert_eq!(hp, Some(75.0));
    }

    #[test]
    fn time_namespace_reads_clock() {
        let (m, _scene, _cam) = manager();
        m.exec("assert(Time.deltaTime() == 0.25)").unwrap();
        m.exec("assert(Time.fixedDeltaTime() > 0)").unwrap();
        m.exec("assert(Time.frameCount() == 1)").unwrap();
    }

    #[test]
    fn camera_set_moves_shared_camera() {
        let (m, _scene, cam) = manager();
        m.exec("Camera.SetPosition(1, 2, 3); Camera.SetFov(60)")
            .unwrap();
        assert_eq!(cam.borrow().position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(cam.borrow().fov, 60.0);
    }

    #[test]
    fn input_press_release_is_writable() {
        let (m, _scene, _cam) = manager();
        m.exec("Input.Press('W'); assert(Input.IsKeyDown('W'))")
            .unwrap();
        m.exec("Input.Release('W'); assert(not Input.IsKeyDown('W'))")
            .unwrap();
    }

    #[test]
    fn raycast_misses_into_empty_space() {
        let (m, _scene, _cam) = manager();
        // Target has no collider, so nothing to hit.
        m.exec("local hit = Physics.Raycast(0,0,0, 1,0,0); assert(hit == false)")
            .unwrap();
    }

    /// #31: a script's `Physics.Raycast` and the engine's `cast_ray` resolve to
    /// the *same* entity for the same ray, because both go through the one rapier
    /// world. The binding casts against the live world shared via the handle.
    #[test]
    fn raycast_matches_engine_cast_through_rapier() {
        use crate::components::{ColliderComponent, ColliderShape};
        use crate::physics::PhysicsWorld;

        let mut raw = Scene::new();
        let target = raw.add_entity("Target".to_string());
        if let Some(mut e) = raw.get_entity_mut(target) {
            e.transform.position = Vec3::new(0.0, 0.0, 5.0);
            e.is_static = true;
            e.collider = Some(ColliderComponent {
                active: true,
                shape: ColliderShape::Box {
                    size: Vec3::splat(2.0),
                },
                is_trigger: false,
                aabb_min: Vec3::ZERO,
                aabb_max: Vec3::ZERO,
            });
        }
        let scene = Rc::new(RefCell::new(raw));
        let input = Rc::new(RefCell::new(InputState::new()));
        let nav = Rc::new(RefCell::new(NavigationGraph::new(
            -10.0, 10.0, -10.0, 10.0, 1.0,
        )));
        let console = Rc::new(RefCell::new(ConsoleLogs::new()));
        let camera = Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0)));
        let time = Rc::new(RefCell::new(Time::new()));
        let mut m = ScriptManager::new(Rc::clone(&scene), input, nav, console, camera, time);
        let physics = Rc::new(RefCell::new(Some(PhysicsWorld::from_scene(
            &scene.borrow(),
        ))));
        m.init_runtime(&physics).expect("runtime inits");

        // Engine path: cast straight down +Z, expect the target.
        let engine = physics
            .borrow()
            .as_ref()
            .unwrap()
            .cast_ray(Vec3::ZERO, Vec3::Z, f32::MAX)
            .map(|(id, _)| id);
        assert_eq!(engine, Some(target), "engine cast should hit the target");

        // Script path: the binding returns (hit, id, dist); pull the id back out.
        let lua_id: u32 = m
            .eval("select(2, Physics.Raycast(0,0,0, 0,0,1))")
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(Some(lua_id), engine, "script raycast must match the engine");
    }
}

mod bindings;

use glam::Vec3;
use mlua::{Lua, RegistryKey, Table};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use crate::core::input::InputState;
use crate::core::scene::Scene;
use crate::navigation::NavigationGraph;
use crate::render::Camera;
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

    /// Initializes a fresh Lua environment and registers all required namespaces
    pub fn init_runtime(&mut self) -> Result<(), String> {
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

        // 2. Transform Namespace
        let transform_table = lua.create_table().map_err(|e| e.to_string())?;
        let scene_clone = Rc::clone(&self.scene);

        transform_table
            .set(
                "GetPosition",
                lua.create_function(move |_, id: u32| {
                    let s = scene_clone.borrow();
                    let pos = s.get_entity(id).map(|e| e.transform.position);
                    match pos {
                        Some(pos) => Ok((pos.x, pos.y, pos.z)),
                        None => Ok((0.0, 0.0, 0.0)),
                    }
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        transform_table
            .set(
                "SetPosition",
                lua.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        e.transform.position = Vec3::new(x, y, z);
                    }
                    s.update_entity_collider(id);
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        transform_table
            .set(
                "GetRotation",
                lua.create_function(move |_, id: u32| {
                    let s = scene_clone.borrow();
                    let rot = s.get_entity(id).map(|e| e.transform.euler_angles());
                    match rot {
                        Some(rot) => Ok((rot.x, rot.y, rot.z)),
                        None => Ok((0.0, 0.0, 0.0)),
                    }
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        transform_table
            .set(
                "SetRotation",
                lua.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        e.transform.set_euler_angles(Vec3::new(x, y, z));
                    }
                    s.update_entity_collider(id);
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        transform_table
            .set(
                "GetScale",
                lua.create_function(move |_, id: u32| {
                    let s = scene_clone.borrow();
                    let scl = s.get_entity(id).map(|e| e.transform.scale);
                    match scl {
                        Some(scl) => Ok((scl.x, scl.y, scl.z)),
                        None => Ok((1.0, 1.0, 1.0)),
                    }
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        transform_table
            .set(
                "SetScale",
                lua.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        e.transform.scale = Vec3::new(x, y, z);
                    }
                    s.update_entity_collider(id);
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        transform_table
            .set(
                "MoveTowards",
                lua.create_function(
                    move |_, (id, tx, ty, tz, step): (u32, f32, f32, f32, f32)| {
                        let mut s = scene_clone.borrow_mut();
                        if let Some(mut e) = s.get_entity_mut(id) {
                            let pos = e.transform.position;
                            let target = Vec3::new(tx, ty, tz);
                            let dir = target - pos;
                            let len = dir.length();
                            if len <= step || len < 0.001 {
                                e.transform.position = target;
                            } else {
                                e.transform.position += dir.normalize() * step;
                            }
                        }
                        s.update_entity_collider(id);
                        Ok(())
                    },
                )
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        lua.globals()
            .set("Transform", transform_table)
            .map_err(|e| e.to_string())?;

        // 2B. Material Namespace
        let material_table = lua.create_table().map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        material_table
            .set(
                "SetMetallic",
                lua.create_function(move |_, (id, val): (u32, f32)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(tex) = &mut e.texture {
                            tex.metallic = val;
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        material_table
            .set(
                "SetRoughness",
                lua.create_function(move |_, (id, val): (u32, f32)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(tex) = &mut e.texture {
                            tex.roughness = val;
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        material_table
            .set(
                "SetMetallicMap",
                lua.create_function(move |_, (id, path): (u32, String)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(tex) = &mut e.texture {
                            tex.metallic_map = Some(path);
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        material_table
            .set(
                "SetRoughnessMap",
                lua.create_function(move |_, (id, path): (u32, String)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(tex) = &mut e.texture {
                            tex.roughness_map = Some(path);
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        material_table
            .set(
                "SetTexture",
                lua.create_function(move |_, (id, path): (u32, String)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(tex) = &mut e.texture {
                            tex.path = path;
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        lua.globals()
            .set("Material", material_table)
            .map_err(|e| e.to_string())?;

        // 3. Animator Namespace
        let animator_table = lua.create_table().map_err(|e| e.to_string())?;
        let scene_clone = Rc::clone(&self.scene);
        let console_clone = Rc::clone(&self.console);
        animator_table
            .set(
                "Play",
                lua.create_function(move |_, (id, clip): (u32, String)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(anim) = &mut e.animator {
                            anim.current_clip = clip.clone();
                            anim.is_playing = true;
                            anim.freeze = false;
                            console_clone
                                .borrow_mut()
                                .info(format!("Entity {} playing animation: {}", e.name, clip));
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        animator_table
            .set(
                "Crossfade",
                lua.create_function(move |_, (id, clip, _duration): (u32, String, f32)| {
                    // Simplify crossfade to standard play in our component
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(anim) = &mut e.animator {
                            anim.current_clip = clip;
                            anim.is_playing = true;
                            anim.freeze = false;
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        animator_table
            .set(
                "Stop",
                lua.create_function(move |_, id: u32| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(anim) = &mut e.animator {
                            anim.is_playing = false;
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        lua.globals()
            .set("Animator", animator_table)
            .map_err(|e| e.to_string())?;

        // 4. Input Namespace
        let input_table = lua.create_table().map_err(|e| e.to_string())?;
        let input_clone = Rc::clone(&self.input);
        input_table
            .set(
                "IsKeyDown",
                lua.create_function(move |_, key_name: String| {
                    Ok(input_clone.borrow().is_key_down(&key_name))
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        lua.globals()
            .set("Input", input_table)
            .map_err(|e| e.to_string())?;

        // 5. Scene Namespace
        let scene_table = lua.create_table().map_err(|e| e.to_string())?;
        let scene_clone = Rc::clone(&self.scene);
        scene_table
            .set(
                "FindEntityByName",
                lua.create_function(move |_, name: String| {
                    let s = scene_clone.borrow();
                    Ok(s.find_entity_by_name(&name))
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        let console_clone = Rc::clone(&self.console);
        scene_table
            .set(
                "DestroyEntity",
                lua.create_function(move |_, id: u32| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        e.active = false;
                        console_clone
                            .borrow_mut()
                            .info(format!("Entity {} destroyed (deactivated)", e.name));
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        lua.globals()
            .set("Scene", scene_table)
            .map_err(|e| e.to_string())?;

        // 6. Navigation Namespace
        let nav_table = lua.create_table().map_err(|e| e.to_string())?;
        let nav_clone = Rc::clone(&self.nav);
        nav_table
            .set(
                "GetNextPathStep",
                lua.create_function(
                    move |_, (cx, cy, cz, tx, ty, tz): (f32, f32, f32, f32, f32, f32)| {
                        let step = nav_clone
                            .borrow()
                            .get_next_path_step(Vec3::new(cx, cy, cz), Vec3::new(tx, ty, tz));
                        Ok((step.x, step.y, step.z))
                    },
                )
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        lua.globals()
            .set("Navigation", nav_table)
            .map_err(|e| e.to_string())?;

        // 6B. NavMeshAgent Namespace
        let agent_table = lua.create_table().map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        agent_table
            .set(
                "SetTarget",
                lua.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(agent) = &mut e.nav_agent {
                            agent.target = Vec3::new(x, y, z);
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        agent_table
            .set(
                "GetTarget",
                lua.create_function(move |_, id: u32| {
                    let s = scene_clone.borrow();
                    if let Some(e) = s.get_entity(id) {
                        if let Some(agent) = &e.nav_agent {
                            let t = agent.target;
                            return Ok((t.x, t.y, t.z));
                        }
                    }
                    Ok((0.0, 0.0, 0.0))
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        agent_table
            .set(
                "SetSpeed",
                lua.create_function(move |_, (id, speed): (u32, f32)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(agent) = &mut e.nav_agent {
                            agent.speed = speed;
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        agent_table
            .set(
                "SetAcceleration",
                lua.create_function(move |_, (id, acc): (u32, f32)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(agent) = &mut e.nav_agent {
                            agent.acceleration = acc;
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        agent_table
            .set(
                "SetStoppingDistance",
                lua.create_function(move |_, (id, dist): (u32, f32)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(agent) = &mut e.nav_agent {
                            agent.stopping_distance = dist;
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        agent_table
            .set(
                "SetRadius",
                lua.create_function(move |_, (id, r): (u32, f32)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(agent) = &mut e.nav_agent {
                            agent.radius = r;
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        agent_table
            .set(
                "IsAtTarget",
                lua.create_function(move |_, id: u32| {
                    let s = scene_clone.borrow();
                    if let Some(e) = s.get_entity(id) {
                        if let Some(agent) = &e.nav_agent {
                            let current_pos = e.transform.position;
                            let to_target = agent.target - current_pos;
                            return Ok(to_target.length() <= agent.stopping_distance);
                        }
                    }
                    Ok(true)
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        agent_table
            .set(
                "GetVelocity",
                lua.create_function(move |_, id: u32| {
                    let s = scene_clone.borrow();
                    if let Some(e) = s.get_entity(id) {
                        if let Some(agent) = &e.nav_agent {
                            let v = agent.velocity;
                            return Ok((v.x, v.y, v.z));
                        }
                    }
                    Ok((0.0, 0.0, 0.0))
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        agent_table
            .set(
                "SetActive",
                lua.create_function(move |_, (id, active): (u32, bool)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(agent) = &mut e.nav_agent {
                            agent.active = active;
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        lua.globals()
            .set("NavMeshAgent", agent_table)
            .map_err(|e| e.to_string())?;

        // 7. Physics Namespace
        let physics_table = lua.create_table().map_err(|e| e.to_string())?;
        let scene_clone = Rc::clone(&self.scene);
        physics_table
            .set(
                "GetVelocity",
                lua.create_function(move |_, id: u32| {
                    let s = scene_clone.borrow();
                    if let Some(e) = s.get_entity(id) {
                        if let Some(rb) = &e.rigidbody {
                            return Ok((rb.velocity.x, rb.velocity.y, rb.velocity.z));
                        }
                    }
                    Ok((0.0, 0.0, 0.0))
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        physics_table
            .set(
                "SetVelocity",
                lua.create_function(move |_, (id, vx, vy, vz): (u32, f32, f32, f32)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(rb) = &mut e.rigidbody {
                            rb.velocity = Vec3::new(vx, vy, vz);
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        physics_table
            .set(
                "AddForce",
                lua.create_function(move |_, (id, fx, fy, fz): (u32, f32, f32, f32)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(rb) = &mut e.rigidbody {
                            if !rb.is_kinematic {
                                let acc = Vec3::new(fx, fy, fz) / rb.mass.max(0.0001);
                                rb.velocity += acc;
                            }
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let scene_clone = Rc::clone(&self.scene);
        physics_table
            .set(
                "SetKinematic",
                lua.create_function(move |_, (id, is_kinematic): (u32, bool)| {
                    let mut s = scene_clone.borrow_mut();
                    if let Some(mut e) = s.get_entity_mut(id) {
                        if let Some(rb) = &mut e.rigidbody {
                            rb.is_kinematic = is_kinematic;
                        }
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        lua.globals()
            .set("Physics", physics_table)
            .map_err(|e| e.to_string())?;

        // 8. Issue #5 namespaces: Health, Time, Camera, writable Input, Physics
        //    Raycast/Shoot, and (dev-only) Debug. Implemented in `bindings.rs` to
        //    keep this monolith from growing without bound.
        bindings::register(
            &lua,
            &self.scene,
            &self.input,
            &self.camera,
            &self.time,
            &self.console,
        )?;

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

        let ids: Vec<u32> = self.entity_scripts.keys().copied().collect();

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
        let ids: Vec<u32> = self
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

    /// Triggers damage callback on the target entity's script if it exists
    pub fn trigger_damage(&mut self, entity_id: u32, amount: f32) {
        let lua = match &self.lua {
            Some(l) => l,
            None => return,
        };

        if let Some(key) = self.entity_scripts.get(&entity_id) {
            if let Ok(table) = lua.registry_value::<Table>(key) {
                if let Ok(damage_fn) = table.get::<_, mlua::Function>("Damage") {
                    if let Err(e) = damage_fn.call::<_, ()>((entity_id, amount)) {
                        self.console.borrow_mut().error(format!(
                            "[Lua Error] Damage callback on entity {} failed: {}",
                            entity_id, e
                        ));
                    }
                } else {
                    // Fallback default: directly reduce health component in scene if script has no custom damage function
                    let mut s = self.scene.borrow_mut();
                    if let Some(mut e_guard) = s.get_entity_mut(entity_id) {
                        let e: &mut crate::core::scene::Entity = &mut e_guard;
                        let name = e.name.clone();
                        if let Some(health) = &mut e.health {
                            health.current_health -= amount;
                            self.console.borrow_mut().info(format!(
                                "Entity {} took {} standard hit (HP: {}/{})",
                                name, amount, health.current_health, health.max_health
                            ));
                            if health.current_health <= 0.0 {
                                health.is_dead = true;
                                if let Some(anim) = &mut e.animator {
                                    anim.current_clip = "Death".to_string();
                                    anim.freeze = true;
                                }
                            }
                        }
                    }
                    drop(s);
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
        m.init_runtime().expect("runtime inits");
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
}

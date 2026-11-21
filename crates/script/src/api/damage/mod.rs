mod cover;
mod deliver;
mod radial;

use std::cell::Cell;
use std::rc::Rc;

use glam::Vec3;
use mlua::{Lua, Table, Value};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_physics::PhysicsWorld;
use rusty_scene::Scene;

pub const MAX_DEPTH: u32 = 16;

/// The damage type when `opts.type` is absent (Unreal's `UDamageType` default).
const GENERIC: &str = "generic";

pub(super) struct Common {
    kind: String,
    instigator: Option<u32>,
    causer: Option<u32>,
    impulse: f32,
}

impl Common {
    fn read(verb: &str, opts: Option<&Table>) -> mlua::Result<Self> {
        let Some(t) = opts else {
            return Ok(Self {
                kind: GENERIC.to_string(),
                instigator: None,
                causer: None,
                impulse: 0.0,
            });
        };
        let impulse = t.get::<Option<f32>>("impulse")?.unwrap_or(0.0);
        finite(verb, "opts.impulse", impulse as f64)?;
        Ok(Self {
            kind: t
                .get::<Option<String>>("type")?
                .unwrap_or_else(|| GENERIC.to_string()),
            instigator: t.get("instigator")?,
            causer: t.get("causer")?,
            impulse,
        })
    }

    fn event(&self, lua: &Lua, kind: &str) -> mlua::Result<Table> {
        let t = lua.create_table()?;
        t.set("kind", kind)?;
        t.set("type", self.kind.as_str())?;
        t.set("instigator", self.instigator)?;
        t.set("causer", self.causer)?;
        Ok(t)
    }
}

fn finite(verb: &str, what: &str, value: f64) -> mlua::Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(mlua::Error::RuntimeError(format!(
            "Damage.{verb}: {what} must be a finite number, got {value}"
        )))
    }
}

fn vec_table(lua: &Lua, v: Vec3) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("x", v.x)?;
    t.set("y", v.y)?;
    t.set("z", v.z)?;
    Ok(t)
}

#[derive(Clone)]
struct Depth(Rc<Cell<u32>>);

struct Nested(Rc<Cell<u32>>);

impl Drop for Nested {
    fn drop(&mut self) {
        self.0.set(self.0.get().saturating_sub(1));
    }
}

fn enter(lua: &Lua, verb: &str) -> mlua::Result<Nested> {
    let existing = lua.app_data_ref::<Depth>().map(|d| d.clone());
    let depth = match existing {
        Some(d) => d,
        None => {
            let d = Depth(Rc::new(Cell::new(0)));
            lua.set_app_data(d.clone());
            d
        }
    };
    let n = depth.0.get();
    if n >= MAX_DEPTH {
        return Err(mlua::Error::RuntimeError(format!(
            "Damage.{verb}: damage nested deeper than {MAX_DEPTH} calls \
             (an OnTakeDamage handler that keeps applying damage)"
        )));
    }
    depth.0.set(n + 1);
    Ok(Nested(depth.0))
}

fn push(lua: &Lua, id: u32, impulse: Vec3, point: Vec3) -> mlua::Result<()> {
    if impulse == Vec3::ZERO {
        return Ok(());
    }
    let physics: Table = lua.globals().get("Physics")?;
    let f: mlua::Function = physics.get("AddImpulseAtPosition")?;
    let (j, p) = (impulse, point);
    f.call::<()>((id, j.x, j.y, j.z, p.x, p.y, p.z))
}

pub fn register(lua: &Lua, scene: ApiCell<Scene>, physics: ApiCell<Option<PhysicsWorld>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    let f = lua.create_function(
        move |lua, (target, amount, opts): (EntityArg, f64, Option<Table>)| {
            let _depth = enter(lua, "Apply")?;
            let amount = finite("Apply", "amount", amount)?;
            let common = Common::read("Apply", opts.as_ref())?;
            let Some(id) = target.get() else {
                return Ok(0.0);
            };
            let event = common.event(lua, "any")?;
            deliver::to_receiver(lua, &*scene.get(lua)?, id, amount, &event)
        },
    );
    put(&table, "Apply", f)?;
    put(&table, "ApplyPoint", apply_point(lua, scene))?;
    put(&table, "ApplyRadial", radial::function(lua, scene, physics))?;
    lua.globals()
        .set("Damage", table)
        .map_err(|e| e.to_string())
}

fn apply_point(lua: &Lua, scene: ApiCell<Scene>) -> mlua::Result<mlua::Function> {
    type Args = (Table, f64, f32, f32, f32, Option<Table>);
    lua.create_function(move |lua, (hit, amount, dx, dy, dz, opts): Args| {
        let _depth = enter(lua, "ApplyPoint")?;
        let amount = finite("ApplyPoint", "amount", amount)?;
        let common = Common::read("ApplyPoint", opts.as_ref())?;
        let Some(id) = hit.get::<Option<u32>>("id")? else {
            return Err(mlua::Error::RuntimeError(
                "Damage.ApplyPoint: the hit has no `id` (pass a RaycastAll or RaycastThrough entry)".into(),
            ));
        };
        let raw = Vec3::new(dx, dy, dz);
        finite("ApplyPoint", "the direction", raw.length() as f64)?;
        let direction = raw.normalize_or_zero();
        let copy = lua.create_table()?;
        for pair in hit.pairs::<Value, Value>() {
            let (k, v) = pair?;
            copy.set(k, v)?;
        }
        if copy.get::<Option<Table>>("point")?.is_none() {
            if let Some(enter) = hit.get::<Option<Table>>("enter")? {
                for key in ["point", "normal", "distance"] {
                    copy.set(key, enter.get::<Value>(key)?)?;
                }
            }
        }
        copy.set("direction", vec_table(lua, direction)?)?;
        let event = common.event(lua, "point")?;
        event.set("hit", copy.clone())?;
        let scene = &*scene.get(lua)?;
        let taken = deliver::to_receiver(lua, scene, id, amount, &event)?;
        if let Some(p) = copy.get::<Option<Table>>("point")? {
            let point = Vec3::new(p.get("x")?, p.get("y")?, p.get("z")?);
            let body = scene.borrow().world.body_carrier(id).unwrap_or(id);
            push(lua, body, direction * common.impulse, point)?;
        }
        Ok(taken)
    })
}

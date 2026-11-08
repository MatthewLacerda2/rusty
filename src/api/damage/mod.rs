//! src/api/damage/mod.rs — `Damage` namespace (#1893): Unreal's damage model.
//!
//! `UGameplayStatics::ApplyDamage` / `ApplyPointDamage` /
//! `ApplyRadialDamageWithFalloff`, delivered to the target's `OnTakeDamage(id,
//! amount, event)`. Unity has no damage system (projects `SendMessage` one), so
//! this follows Unreal. The engine stores no health: what damage *does* is the
//! game's Lua; the engine gives the delivery, the hit information and the
//! radial query.
//!
//! - **Who receives it** ([`deliver`]): the nearest active ancestor-or-self of
//!   the entity named (or hit) whose live script defines `OnTakeDamage` — the
//!   bubbling walk the UI pointer callbacks take (`ui::events::nearest`), so a
//!   hitbox under a bone reaches the character's script.
//! - **When**: synchronously, inside the call, through the live instance index
//!   (`scripting::instances`, what `Scene.GetScript` reads). No queue. A handler
//!   may apply damage itself; past [`MAX_DEPTH`] nested calls the next one is a
//!   Lua error, and every call under it unwinds with it.
//! - **Radial** ([`radial`]): an overlap, grouped by receiver, nearest point per
//!   group, a cover ray, then the falloff — nearest first, ties by entity id.
//! - **Impulse**: `opts.impulse` pushes the struck body through
//!   `Physics.AddImpulseAtPosition` itself, the same code a script calls.

mod deliver;
mod radial;

use std::cell::Cell;
use std::rc::Rc;

use glam::Vec3;
use mlua::{Lua, Table, Value};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::physics::PhysicsWorld;
use crate::scene::Scene;

/// How many `Damage.*` calls may nest (a handler applying damage, whose target's
/// handler applies damage, …) before the next one is a Lua error.
pub const MAX_DEPTH: u32 = 16;

/// The damage type when `opts.type` is absent (Unreal's `UDamageType` default).
const GENERIC: &str = "generic";

/// The options every verb takes: `type`, `instigator`, `causer`, `impulse`.
pub(super) struct Common {
    kind: String,
    instigator: Option<u32>,
    causer: Option<u32>,
    impulse: f32,
}

impl Common {
    /// Read the shared fields of `opts` (absent: the defaults).
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

    /// A fresh `event` table of `kind` (`"any"`, `"point"`, `"radial"`).
    fn event(&self, lua: &Lua, kind: &str) -> mlua::Result<Table> {
        let t = lua.create_table()?;
        t.set("kind", kind)?;
        t.set("type", self.kind.as_str())?;
        t.set("instigator", self.instigator)?;
        t.set("causer", self.causer)?;
        Ok(t)
    }
}

/// A Lua error unless `value` is a finite number.
fn finite(verb: &str, what: &str, value: f64) -> mlua::Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(mlua::Error::RuntimeError(format!(
            "Damage.{verb}: {what} must be a finite number, got {value}"
        )))
    }
}

/// `{x, y, z}`.
fn vec_table(lua: &Lua, v: Vec3) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("x", v.x)?;
    t.set("y", v.y)?;
    t.set("z", v.z)?;
    Ok(t)
}

/// The VM's nesting count, made on first use.
#[derive(Clone)]
struct Depth(Rc<Cell<u32>>);

/// One level of nesting, released when dropped (an error unwinding included).
struct Nested(Rc<Cell<u32>>);

impl Drop for Nested {
    fn drop(&mut self) {
        self.0.set(self.0.get().saturating_sub(1));
    }
}

/// Enter one `Damage.<verb>` call: an error past [`MAX_DEPTH`] nested ones.
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

/// Push `id`'s body with `impulse` at `point`, through the script verb
/// `Physics.AddImpulseAtPosition` (a no-op on a kinematic or bodiless entity).
fn push(lua: &Lua, id: u32, impulse: Vec3, point: Vec3) -> mlua::Result<()> {
    if impulse == Vec3::ZERO {
        return Ok(());
    }
    let physics: Table = lua.globals().get("Physics")?;
    let f: mlua::Function = physics.get("AddImpulseAtPosition")?;
    let (j, p) = (impulse, point);
    f.call::<()>((id, j.x, j.y, j.z, p.x, p.y, p.z))
}

/// Register `Damage` onto `lua`.
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

/// `ApplyPoint(hit, amount, dx, dy, dz [, opts])`.
fn apply_point(lua: &Lua, scene: ApiCell<Scene>) -> mlua::Result<mlua::Function> {
    type Args = (Table, f64, f32, f32, f32, Option<Table>);
    lua.create_function(move |lua, (hit, amount, dx, dy, dz, opts): Args| {
        let _depth = enter(lua, "ApplyPoint")?;
        let amount = finite("ApplyPoint", "amount", amount)?;
        let common = Common::read("ApplyPoint", opts.as_ref())?;
        let Some(id) = hit.get::<Option<u32>>("id")? else {
            return Err(mlua::Error::RuntimeError(
                "Damage.ApplyPoint: the hit has no `id` (pass a RaycastAll entry)".into(),
            ));
        };
        let raw = Vec3::new(dx, dy, dz);
        finite("ApplyPoint", "the direction", raw.length() as f64)?;
        let direction = raw.normalize_or_zero();
        // A copy, so a caller's reused `RaycastAllNonAlloc` entry is not written.
        let copy = lua.create_table()?;
        for pair in hit.pairs::<Value, Value>() {
            let (k, v) = pair?;
            copy.set(k, v)?;
        }
        copy.set("direction", vec_table(lua, direction)?)?;
        let event = common.event(lua, "point")?;
        event.set("hit", copy)?;
        let scene = &*scene.get(lua)?;
        let taken = deliver::to_receiver(lua, scene, id, amount, &event)?;
        // The push needs where the shot landed; a hit without a `point` gets none.
        if let Some(p) = hit.get::<Option<Table>>("point")? {
            let point = Vec3::new(p.get("x")?, p.get("y")?, p.get("z")?);
            let body = scene.borrow().world.body_carrier(id).unwrap_or(id);
            push(lua, body, direction * common.impulse, point)?;
        }
        Ok(taken)
    })
}

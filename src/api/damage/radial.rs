//! src/api/damage/radial.rs — `Damage.ApplyRadial`, Unreal's
//! `ApplyRadialDamageWithFalloff` (#1893).
//!
//! 1. **Candidates**: every solid (non-trigger) collider overlapping the sphere of
//!    radius `outer`, on `layer_mask`, grouped by who it belongs to: its damage
//!    receiver ([`deliver::receiver`]), else its body, else itself. A character
//!    with fifteen hitboxes is one group, damaged once.
//! 2. **Distance**: the nearest of the group's colliders' closest points.
//! 3. **Cover** (Unreal's `ComponentIsDamageableFrom`): a ray from the origin to
//!    that point, or to that collider's bounds centre, must meet nothing on
//!    `blocking_mask` first. The group itself and the `causer` never block.
//! 4. **Falloff**: full inside `inner`, `min` at `outer`, and between them
//!    `min + (amount - min) · (1 - t)^falloff` with `t` the fraction of the way out.
//!
//! Groups are processed nearest first, ties by entity id, so two runs over one
//! scene give the same list. Every dynamic body in a clear group is pushed by
//! `impulse` scaled by the same falloff, away from the origin, damaged or not.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::{deliver, enter, finite, push, vec_table, Common};
use crate::api::ApiCell;
use crate::physics::{is_hittable, PhysicsWorld};
use crate::scene::{layer_in_mask, Scene};
use crate::ui::events::is_under;

/// How short of its target a cover ray may stop and still count as clear (m).
const SKIN: f32 = 1e-3;

/// One `ApplyRadial` call's shape, read from its arguments.
struct Blast {
    origin: Vec3,
    amount: f64,
    inner: f32,
    outer: f32,
    min: f64,
    falloff: f64,
    layer_mask: Option<u32>,
    blocking_mask: Option<u32>,
}

impl Blast {
    /// Validate `opts`: `outer` is required; `0 <= inner <= outer`, `falloff > 0`.
    fn read(origin: Vec3, amount: f64, opts: &Table) -> mlua::Result<Self> {
        let num = |key: &str, default: Option<f64>| -> mlua::Result<f64> {
            let v = opts.get::<Option<f64>>(key)?.or(default).ok_or_else(|| {
                mlua::Error::RuntimeError(format!("Damage.ApplyRadial: opts.{key} is required"))
            })?;
            finite("ApplyRadial", &format!("opts.{key}"), v)
        };
        let outer = num("outer", None)?;
        let inner = num("inner", Some(0.0))?;
        let falloff = num("falloff", Some(1.0))?;
        if !(0.0..=outer).contains(&inner) || falloff <= 0.0 {
            return Err(mlua::Error::RuntimeError(format!(
                "Damage.ApplyRadial: needs 0 <= inner <= outer and falloff > 0 \
                 (inner {inner}, outer {outer}, falloff {falloff})"
            )));
        }
        Ok(Self {
            origin,
            amount: finite("ApplyRadial", "amount", amount)?,
            inner: inner as f32,
            outer: outer as f32,
            min: num("min", Some(0.0))?,
            falloff,
            layer_mask: opts.get("layer_mask")?,
            blocking_mask: opts.get("blocking_mask")?,
        })
    }

    /// The fraction of the full blast at `distance`: 1 inside `inner`, falling to
    /// 0 at `outer`; `None` past it.
    fn scale(&self, distance: f32) -> Option<f64> {
        if distance > self.outer {
            return None;
        }
        if distance <= self.inner {
            return Some(1.0);
        }
        let t = ((distance - self.inner) / (self.outer - self.inner)) as f64;
        Some((1.0 - t).max(0.0).powf(self.falloff))
    }
}

/// One collider the blast reached.
struct Row {
    owner: u32,
    receives: bool,
    id: u32,
    body: u32,
    distance: f32,
    point: Vec3,
}

/// One group the blast damages: its nearest row, and each body's nearest row.
struct Group {
    nearest: Row,
    bodies: Vec<(u32, Vec3)>,
}

/// Whether `id`'s collider is a solid one on `mask` (`None`: every layer).
fn solid_on(scene: &Scene, id: u32, mask: Option<u32>) -> bool {
    let world = &scene.world;
    let trigger =
        world.collider(id).is_some_and(|c| c.is_trigger) && !world.has_character_controller(id);
    is_hittable(scene, id) && !trigger && mask.is_none_or(|m| layer_in_mask(world.layer(id), m))
}

/// Every collider the sphere reaches, as rows sorted by (owner, distance, id).
fn rows(lua: &Lua, scene: &Scene, physics: &PhysicsWorld, blast: &Blast) -> Vec<Row> {
    let ids = physics.overlap_sphere(blast.origin, blast.outer, |id| {
        solid_on(scene, id, blast.layer_mask)
    });
    let mut rows: Vec<Row> = ids
        .into_iter()
        .filter_map(|id| {
            let point = physics.closest_point_on(id, blast.origin)?;
            let body = scene.world.body_carrier(id).unwrap_or(id);
            let receiver = deliver::receiver(lua, scene, id);
            Some(Row {
                owner: receiver.unwrap_or(body),
                receives: receiver.is_some(),
                id,
                body,
                distance: point.distance(blast.origin),
                point,
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        (a.owner, a.distance, a.id)
            .partial_cmp(&(b.owner, b.distance, b.id))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows
}

/// Whether a ray from the origin reaches `target` without meeting a blocker:
/// a solid collider on `blocking_mask` that is not under `owner` or the causer.
fn clear(
    scene: &Scene,
    physics: &PhysicsWorld,
    (blast, causer): (&Blast, Option<u32>),
    owner: u32,
    target: Vec3,
) -> bool {
    let to = target - blast.origin;
    let length = to.length();
    if length <= SKIN {
        return true;
    }
    let world = &scene.world;
    let blocks = |id: u32| {
        solid_on(scene, id, blast.blocking_mask)
            && !is_under(world, id, owner)
            && causer.is_none_or(|c| !is_under(world, id, c))
    };
    physics
        .raycast_hit(blast.origin, to, length, blocks)
        .is_none_or(|hit| hit.distance >= length - SKIN)
}

/// The groups the blast damages, nearest first (ties by owner id).
fn groups(
    lua: &Lua,
    scene: &Scene,
    physics: &PhysicsWorld,
    blast: &Blast,
    causer: Option<u32>,
) -> Vec<Group> {
    let mut out: Vec<Group> = Vec::new();
    for row in rows(lua, scene, physics, blast) {
        match out.last_mut() {
            Some(g) if g.nearest.owner == row.owner => {
                if !g.bodies.iter().any(|(b, _)| *b == row.body) {
                    g.bodies.push((row.body, row.point));
                }
            }
            _ => out.push(Group {
                bodies: vec![(row.body, row.point)],
                nearest: row,
            }),
        }
    }
    out.retain(|g| {
        let n = &g.nearest;
        let centre = scene
            .world
            .collider_bounds(n.id)
            .map(|(lo, hi)| (lo + hi) * 0.5);
        let ctx = (blast, causer);
        clear(scene, physics, ctx, n.owner, n.point)
            || centre.is_some_and(|c| clear(scene, physics, ctx, n.owner, c))
    });
    out.sort_by(|a, b| {
        (a.nearest.distance, a.nearest.owner)
            .partial_cmp(&(b.nearest.distance, b.nearest.owner))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// `ApplyRadial(x, y, z, amount, opts)` — `{ {id, amount, distance}, … }`, one
/// per receiver that took damage, nearest first.
pub(super) fn function(
    lua: &Lua,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> mlua::Result<mlua::Function> {
    type Args = (f32, f32, f32, f64, Table);
    lua.create_function(move |lua, (x, y, z, amount, opts): Args| {
        let _depth = enter(lua, "ApplyRadial")?;
        let origin = Vec3::new(x, y, z);
        finite("ApplyRadial", "the origin", origin.length() as f64)?;
        let blast = Blast::read(origin, amount, &opts)?;
        let common = Common::read("ApplyRadial", Some(&opts))?;
        let scene = &*scene.get(lua)?;
        let found = gather(lua, scene, &*physics.get(lua)?, &blast, common.causer);
        let results = lua.create_table()?;
        for g in found {
            let Some(scale) = blast.scale(g.nearest.distance) else {
                continue;
            };
            if g.nearest.receives {
                let event = common.event(lua, "radial")?;
                event.set("origin", vec_table(lua, origin)?)?;
                event.set("distance", g.nearest.distance)?;
                let damage = blast.min + (blast.amount - blast.min) * scale;
                let taken = deliver::call(lua, g.nearest.owner, damage, &event)?;
                let row = lua.create_table()?;
                row.set("id", g.nearest.owner)?;
                row.set("amount", taken)?;
                row.set("distance", g.nearest.distance)?;
                results.push(row)?;
            }
            let strength = common.impulse * scale as f32;
            for (body, point) in g.bodies {
                let away = (point - origin).normalize_or_zero();
                push(lua, body, away * strength, point)?;
            }
        }
        Ok(results)
    })
}

/// [`groups`] under the scene and physics borrows, released before any handler runs.
fn gather(
    lua: &Lua,
    scene: &RefCell<Scene>,
    physics: &RefCell<Option<PhysicsWorld>>,
    blast: &Blast,
    causer: Option<u32>,
) -> Vec<Group> {
    let physics = physics.borrow();
    let Some(world) = physics.as_ref() else {
        return Vec::new();
    };
    groups(lua, &scene.borrow(), world, blast, causer)
}

//! crates/script/src/api/damage/radial.rs — `Damage.ApplyRadial`, Unreal's
//! `ApplyRadialDamageWithFalloff` (#1893).
//!
//! 1. **Candidates**: every solid (non-trigger) collider overlapping the sphere of
//!    radius `outer`, on `layer_mask`, outside the `causer`'s hierarchy (Unreal
//!    passes the causer as an ignored actor), grouped by who it belongs to: its
//!    damage receiver ([`deliver::receiver`]), else its body, else itself. A
//!    character with fifteen hitboxes is one group, damaged once.
//! 2. **Cover**, per collider (Unreal's `ComponentIsDamageableFrom`, [`cover`]):
//!    a collider is reached when a ray to its closest point, or to its bounds
//!    centre, meets no blocker first. A group is reached when any of its
//!    colliders is: a bot crouched behind a crate with its head in view.
//! 3. **Distance**: the nearest *reached* collider's closest point.
//! 4. **Falloff**: full inside `inner`, `min` at `outer`, and between them
//!    `min + (amount - min) · (1 - t)^falloff` with `t` the fraction of the way out.
//!
//! Groups are processed nearest first, ties by entity id, so two runs over one
//! scene give the same list; a receiver a nested blast deactivated is skipped.
//! Every dynamic body in a reached group is pushed by `impulse` scaled by the same
//! falloff, away from the origin (from its collider's bounds centre when the
//! origin is on it), damaged or not.
//!
//! The damage sense (#1900) hears each blast **once per perceiver**: every
//! reached group's struck collider leads to its perceiver
//! (`ai::perception::perceiver_of`, a walk up from the struck body, as for
//! `Apply`), which senses the **largest** amount any of its groups took, at
//! the blast origin (or where a live instigator stands). A bot of five hitboxes
//! and no `OnTakeDamage` is one report, not five.

use std::cell::RefCell;
use std::collections::BTreeMap;

use glam::Vec3;
use mlua::{Lua, Table};

use super::cover::{clear, solid_on, SKIN};
use super::{deliver, enter, finite, push, vec_table, Common};
use crate::api::ApiCell;
use rusty_physics::PhysicsWorld;
use rusty_scene::Scene;
use rusty_ui::events::is_under;

/// One `ApplyRadial` call's shape, read from its arguments.
pub(super) struct Blast {
    pub(super) origin: Vec3,
    amount: f64,
    inner: f32,
    outer: f32,
    min: f64,
    falloff: f64,
    layer_mask: Option<u32>,
    pub(super) blocking_mask: Option<u32>,
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

/// One group the blast reaches: its nearest reached row, and each body's
/// nearest row with that row's collider (for the push).
struct Group {
    nearest: Row,
    bodies: Vec<(u32, Vec3, u32)>,
}

/// Every collider the sphere reaches outside the causer's hierarchy, as rows
/// sorted by (owner, distance, id).
fn rows(
    lua: &Lua,
    scene: &Scene,
    physics: &PhysicsWorld,
    blast: &Blast,
    causer: Option<u32>,
) -> Vec<Row> {
    let world = &scene.world;
    let ids = physics.overlap_sphere(blast.origin, blast.outer, |id| {
        solid_on(scene, id, blast.layer_mask) && causer.is_none_or(|c| !is_under(world, id, c))
    });
    let mut rows: Vec<Row> = ids
        .into_iter()
        .filter_map(|id| {
            let point = physics.closest_point_on(id, blast.origin)?;
            let body = world.body_carrier(id).unwrap_or(id);
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

/// Whether the blast reaches `row`'s collider: its closest point or its bounds
/// centre is clear of cover.
fn reached(scene: &Scene, physics: &PhysicsWorld, ctx: (&Blast, Option<u32>), row: &Row) -> bool {
    let centre = scene
        .world
        .collider_bounds(row.id)
        .map(|(lo, hi)| (lo + hi) * 0.5);
    clear(scene, physics, ctx, row.owner, row.point)
        || centre.is_some_and(|c| clear(scene, physics, ctx, row.owner, c))
}

/// One owner's rows (nearest first) as a group, when any of them is reached.
fn group(
    scene: &Scene,
    physics: &PhysicsWorld,
    ctx: (&Blast, Option<u32>),
    rows: Vec<Row>,
) -> Option<Group> {
    let mut bodies: Vec<(u32, Vec3, u32)> = Vec::new();
    for r in &rows {
        if !bodies.iter().any(|(b, _, _)| *b == r.body) {
            bodies.push((r.body, r.point, r.id));
        }
    }
    let nearest = rows.into_iter().find(|r| reached(scene, physics, ctx, r))?;
    Some(Group { nearest, bodies })
}

/// The groups the blast reaches, nearest first (ties by owner id).
fn groups(
    lua: &Lua,
    scene: &Scene,
    physics: &PhysicsWorld,
    blast: &Blast,
    causer: Option<u32>,
) -> Vec<Group> {
    let mut by_owner: Vec<Vec<Row>> = Vec::new();
    for row in rows(lua, scene, physics, blast, causer) {
        match by_owner.last_mut() {
            Some(g) if g[0].owner == row.owner => g.push(row),
            _ => by_owner.push(vec![row]),
        }
    }
    let mut out: Vec<Group> = by_owner
        .into_iter()
        .filter_map(|rows| group(scene, physics, (blast, causer), rows))
        .collect();
    out.sort_by(|a, b| {
        (a.nearest.distance, a.nearest.owner)
            .partial_cmp(&(b.nearest.distance, b.nearest.owner))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// The unit push direction for a body struck at `point` (on collider `id`):
/// away from the origin, or from the collider's bounds centre when the origin
/// lies on or in it.
fn away(scene: &RefCell<Scene>, origin: Vec3, point: Vec3, id: u32) -> Vec3 {
    if point.distance(origin) > SKIN {
        return (point - origin).normalize_or_zero();
    }
    let bounds = scene.borrow().world.collider_bounds(id);
    bounds.map_or(Vec3::ZERO, |(lo, hi)| {
        ((lo + hi) * 0.5 - origin).normalize_or_zero()
    })
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
        let mut sensed: BTreeMap<u32, f64> = BTreeMap::new();
        for g in found {
            let Some(scale) = blast.scale(g.nearest.distance) else {
                continue;
            };
            let owner = g.nearest.owner;
            let active = scene.borrow().world.is_active_in_hierarchy(owner);
            let damage = blast.min + (blast.amount - blast.min) * scale;
            let perceiver = rusty_ai::perception::perceiver_of(&scene.borrow(), g.nearest.id);
            if let Some(p) = perceiver {
                let most = sensed.entry(p).or_insert(damage);
                *most = most.max(damage);
            }
            if g.nearest.receives && active {
                let event = common.event(lua, "radial")?;
                event.set("origin", vec_table(lua, origin)?)?;
                event.set("distance", g.nearest.distance)?;
                let taken = deliver::call(lua, g.nearest.owner, damage, &event)?;
                let row = lua.create_table()?;
                row.set("id", g.nearest.owner)?;
                row.set("amount", taken)?;
                row.set("distance", g.nearest.distance)?;
                results.push(row)?;
            }
            let strength = common.impulse * scale as f32;
            for (body, point, id) in g.bodies {
                let dir = away(scene, origin, point, id);
                push(lua, body, dir * strength, point)?;
            }
        }
        for (p, damage) in sensed {
            let hit = (damage, common.kind.as_str());
            deliver::sense(scene, p, common.instigator, origin, hit);
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

//! The places a spec names (#1892): `querier` (always the caller), and whatever
//! the `contexts` table gives, each an entity id, a point `{x, y, z}` or a list of
//! points. Resolved once per call into [`Anchor`]s, so a destroyed or inactive
//! entity is an error naming its field before anything runs.

use std::collections::BTreeMap;

use glam::Vec3;
use mlua::{Table, Value};

use super::fields::{fail, is_point, point, points};
use crate::ai::query::Anchor;
use crate::scene::Scene;

/// One named context.
#[derive(Clone, Debug)]
pub enum Context {
    One(Anchor),
    Many(Vec<Vec3>),
}

/// Every context a call can name.
pub struct Contexts(BTreeMap<String, Context>);

/// The name the querier always goes by.
pub const QUERIER: &str = "querier";

impl Contexts {
    /// The querier plus `table`'s entries.
    pub fn read(scene: &Scene, querier: Anchor, table: Option<Table>) -> mlua::Result<Self> {
        let mut named = BTreeMap::new();
        named.insert(QUERIER.to_string(), Context::One(querier));
        for pair in table.iter().flat_map(|t| t.pairs::<Value, Value>()) {
            let (key, value) = pair?;
            let Value::String(key) = key else {
                return fail("contexts", "keys must be names");
            };
            let key = key.to_str()?.to_string();
            let path = format!("contexts.{key}");
            if key == QUERIER {
                return fail(&path, "'querier' is always the caller and cannot be set");
            }
            let context = match &value {
                Value::Integer(i) => {
                    let id = u32::try_from(*i).or_else(|_| fail(&path, "expected an entity id"))?;
                    Context::One(entity(scene, id, &path)?)
                }
                v if is_point(v)? => Context::One(Anchor::point(point(v, &path)?)),
                v => Context::Many(points(v, &path)?),
            };
            named.insert(key, context);
        }
        Ok(Self(named))
    }

    /// The single place `name` names, for the field at `path`.
    pub fn one(&self, name: &str, path: &str) -> mlua::Result<Anchor> {
        match self.0.get(name) {
            Some(Context::One(a)) => Ok(*a),
            Some(Context::Many(_)) => fail(
                path,
                format!("context '{name}' is a list of points, not one place"),
            ),
            None => fail(path, format!("unknown context '{name}'")),
        }
    }

    /// The points `name` names (one place reads as a list of one).
    pub fn many(&self, name: &str, path: &str) -> mlua::Result<Vec<Vec3>> {
        match self.0.get(name) {
            Some(Context::Many(p)) => Ok(p.clone()),
            Some(Context::One(a)) => Ok(vec![a.position]),
            None => fail(path, format!("unknown context '{name}'")),
        }
    }
}

/// An entity as a context ([`Anchor::of_entity`]: where it stands and its
/// world forward). A destroyed or inactive entity is an error.
pub fn entity(scene: &Scene, id: u32, path: &str) -> mlua::Result<Anchor> {
    if !scene.world.is_active_in_hierarchy(id) {
        return fail(path, format!("entity {id} is destroyed or inactive"));
    }
    Ok(Anchor::of_entity(scene, id))
}

//! A Lua spec table into an [`ai::query::QuerySpec`](QuerySpec) (#1892), every
//! name resolved against the call's [`Contexts`] and every mistake an error
//! naming its field.

use mlua::{Table, Value};

use super::contexts::{Contexts, QUERIER};
use super::fields::{at, boolean, count, fail, number, only, points, text};
use crate::ai::query::{Generator, QuerySpec, DEFAULT_PROJECT, MAX_ITEMS};

/// Every test kind, the one key that names a test's measure.
pub(super) const KINDS: [&str; 7] = [
    "distance",
    "dot",
    "trace",
    "view",
    "path",
    "separation",
    "near_points",
];

/// Read the whole spec.
pub fn read(spec: &Table, contexts: &Contexts) -> mlua::Result<QuerySpec> {
    only(spec, "spec", &["generator", "tests"])?;
    let Value::Table(g) = spec.get::<Value>("generator")? else {
        return fail("generator", "required: a table with donut, grid or points");
    };
    only(&g, "generator", &["donut", "grid", "points", "project"])?;
    let generator = generator(&g, contexts)?;
    if generator.count() > MAX_ITEMS {
        let n = generator.count();
        return fail(
            "generator",
            format!("{n} items, past the {MAX_ITEMS} a query may make"),
        );
    }
    let project = number(&g, "generator", "project", Some(DEFAULT_PROJECT))?.max(0.0);
    let mut tests = Vec::new();
    match spec.get::<Value>("tests")? {
        Value::Nil => {}
        Value::Table(list) => {
            for (i, entry) in list.sequence_values::<Value>().enumerate() {
                let path = format!("tests[{}]", i + 1);
                let Value::Table(entry) = entry? else {
                    return fail(&path, "expected a test table");
                };
                tests.push(super::entry::test(&entry, &path, contexts)?);
            }
        }
        other => {
            return fail(
                "tests",
                format!("expected a list, got {}", other.type_name()),
            )
        }
    }
    Ok(QuerySpec {
        generator,
        project,
        tests,
    })
}

fn generator(g: &Table, contexts: &Contexts) -> mlua::Result<Generator> {
    let kinds = ["donut", "grid", "points"].map(|k| g.contains_key(k).unwrap_or(false));
    if kinds.iter().filter(|k| **k).count() != 1 {
        return fail("generator", "needs exactly one of donut, grid or points");
    }
    if let Value::Table(d) = g.get::<Value>("donut")? {
        return donut(&d, contexts);
    }
    if let Value::Table(t) = g.get::<Value>("grid")? {
        return grid(&t, contexts);
    }
    Ok(Generator::Points(point_list(
        g,
        "generator",
        "points",
        contexts,
    )?))
}

/// `generator.donut`.
fn donut(d: &Table, contexts: &Contexts) -> mlua::Result<Generator> {
    let path = "generator.donut";
    only(d, path, &["context", "inner", "outer", "rings", "per_ring"])?;
    let center = place(d, path, "context", QUERIER, contexts)?.position;
    let inner = number(d, path, "inner", Some(0.0))?;
    let outer = number(d, path, "outer", None)?;
    if inner < 0.0 || outer < inner {
        return fail(
            path,
            format!("needs 0 ≤ inner ≤ outer, got {inner} and {outer}"),
        );
    }
    let rings = count(d, path, "rings", 1, 1)?;
    let per_ring = count(d, path, "per_ring", 8, 1)?;
    Ok(Generator::Donut {
        center,
        inner,
        outer,
        rings,
        per_ring,
    })
}

/// `generator.grid`.
fn grid(t: &Table, contexts: &Contexts) -> mlua::Result<Generator> {
    let path = "generator.grid";
    only(t, path, &["context", "half_size", "spacing"])?;
    let center = place(t, path, "context", QUERIER, contexts)?.position;
    let half_size = number(t, path, "half_size", None)?;
    let spacing = number(t, path, "spacing", Some(1.0))?;
    if half_size < 0.0 || spacing <= 0.0 {
        return fail(path, "needs half_size ≥ 0 and spacing > 0");
    }
    Ok(Generator::Grid {
        center,
        half_size,
        spacing,
    })
}

/// The single place `t[key]` names (default `default`).
pub fn place(
    t: &Table,
    path: &str,
    key: &str,
    default: &str,
    contexts: &Contexts,
) -> mlua::Result<crate::ai::query::Anchor> {
    let name = text(t, path, key)?.unwrap_or_else(|| default.to_string());
    contexts.one(&name, &at(path, key))
}

/// `t[key]` as points: a context name, or a list given inline.
pub fn point_list(
    t: &Table,
    path: &str,
    key: &str,
    contexts: &Contexts,
) -> mlua::Result<Vec<glam::Vec3>> {
    let here = at(path, key);
    match t.get::<Value>(key)? {
        Value::String(name) => contexts.many(&name.to_str()?, &here),
        v @ Value::Table(_) => points(&v, &here),
        other => fail(
            &here,
            format!(
                "expected a context name or a list of points, got {}",
                other.type_name()
            ),
        ),
    }
}

/// `t[key]` as a yes/no answer (default `true`).
pub fn wanted(t: &Table, path: &str, key: &str) -> mlua::Result<bool> {
    boolean(t, path, key, true)
}

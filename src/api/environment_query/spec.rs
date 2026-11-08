//! A Lua spec table into an [`ai::query::QuerySpec`](QuerySpec) (#1892), every
//! name resolved against the call's [`Contexts`] and every mistake an error
//! naming its field.

use mlua::{Table, Value};

use super::contexts::{Contexts, QUERIER};
use super::fields::{at, boolean, count, fail, number, only, points, text};
use crate::ai::query::{
    Curve, Filter, Generator, Measure, QuerySpec, Score, Test, DEFAULT_PROJECT, MAX_ITEMS,
};

/// Every test kind, the one key that names a test's measure.
const KINDS: [&str; 7] = [
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
                tests.push(test(&entry, &path, contexts)?);
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

/// One test entry: its measure (the one kind key), then what it filters and scores.
fn test(entry: &Table, path: &str, contexts: &Contexts) -> mlua::Result<Test> {
    let mut allowed = KINDS.to_vec();
    allowed.extend(["purpose", "score", "min", "max"]);
    only(entry, path, &allowed)?;
    let present: Vec<&str> = KINDS
        .into_iter()
        .filter(|k| entry.contains_key(*k).unwrap_or(false))
        .collect();
    let [kind] = present[..] else {
        return fail(path, format!("needs exactly one of {}", KINDS.join(", ")));
    };
    let (measure, wanted) = measure(entry, kind, &at(path, kind), contexts)?;
    let range = (
        optional_number(entry, path, "min")?,
        optional_number(entry, path, "max")?,
    );
    let score = score_of(entry, path)?;
    let (filters, scores) = purpose(entry, path, wanted.is_some(), range, score.is_some())?;
    let filter = match (filters, wanted, range) {
        (false, ..) => None,
        (true, Some(want), _) => Some(Filter::Is(want)),
        (true, None, (None, None)) => return fail(path, "a filter on a number needs min or max"),
        (true, None, (min, max)) => Some(Filter::Range { min, max }),
    };
    if filter.is_none() && !scores {
        return fail(
            path,
            "neither filters nor scores: give purpose, min/max or score",
        );
    }
    let score = scores.then(|| {
        score.unwrap_or(Score {
            curve: Curve::Linear,
            weight: 1.0,
        })
    });
    Ok(Test {
        measure,
        filter,
        score,
    })
}

/// The entry's `score` table, if any.
fn score_of(entry: &Table, path: &str) -> mlua::Result<Option<Score>> {
    match entry.get::<Value>("score")? {
        Value::Nil => Ok(None),
        Value::Table(s) => Ok(Some(score(&s, &at(path, "score"))?)),
        other => fail(
            &at(path, "score"),
            format!("expected a table, got {}", other.type_name()),
        ),
    }
}

/// Whether the entry filters and whether it scores. Unspecified: a yes/no test
/// with no score filters, `min`/`max` filter, a score scores.
fn purpose(
    entry: &Table,
    path: &str,
    yes_no: bool,
    range: (Option<f32>, Option<f32>),
    has_score: bool,
) -> mlua::Result<(bool, bool)> {
    match text(entry, path, "purpose")?.as_deref() {
        None => Ok((range != (None, None) || (yes_no && !has_score), has_score)),
        Some("filter") => Ok((true, false)),
        Some("score") => Ok((false, true)),
        Some("filter_score") => Ok((true, true)),
        Some(other) => fail(
            &at(path, "purpose"),
            format!("unknown purpose '{other}' (expected filter, score, filter_score)"),
        ),
    }
}

/// The measure `kind` names, and for a yes/no test the answer its filter keeps.
fn measure(
    entry: &Table,
    kind: &str,
    path: &str,
    contexts: &Contexts,
) -> mlua::Result<(Measure, Option<bool>)> {
    if kind == "path" {
        return match text(entry, "", "path")
            .or_else(|_| fail(path, "expected \"exists\" or \"length\""))?
        {
            Some(m) if m == "exists" => Ok((Measure::PathExists, Some(true))),
            Some(m) if m == "length" => Ok((Measure::PathLength, None)),
            _ => fail(path, "expected \"exists\" or \"length\""),
        };
    }
    let Value::Table(t) = entry.get::<Value>(kind)? else {
        return fail(path, "expected a table");
    };
    super::measures::read(&t, kind, path, contexts)
}

fn score(s: &Table, path: &str) -> mlua::Result<Score> {
    only(s, path, &["curve", "weight"])?;
    let curve = match text(s, path, "curve")? {
        None => Curve::Linear,
        Some(name) => Curve::parse(&name).map_or_else(
            || {
                fail(
                    &at(path, "curve"),
                    format!("unknown curve '{name}' (expected {})", Curve::names()),
                )
            },
            Ok,
        )?,
    };
    let weight = number(s, path, "weight", Some(1.0))?;
    Ok(Score { curve, weight })
}

fn optional_number(t: &Table, path: &str, key: &str) -> mlua::Result<Option<f32>> {
    match t.get::<Value>(key)? {
        Value::Nil => Ok(None),
        _ => number(t, path, key, None).map(Some),
    }
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

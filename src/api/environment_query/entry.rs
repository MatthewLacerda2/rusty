//! One entry of a spec's `tests` list (#1892): its measure, what it filters and
//! what it scores, each contradiction an error naming its field.

use mlua::{Table, Value};

use super::contexts::Contexts;
use super::fields::{at, fail, number, only, text};
use super::spec::KINDS;
use crate::ai::query::{Curve, Filter, Measure, Score, Test};

/// One test entry: its measure (the one kind key), then what it filters and scores.
pub fn test(entry: &Table, path: &str, contexts: &Contexts) -> mlua::Result<Test> {
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
    let (measure, yes_no) = measure(entry, kind, &at(path, kind), contexts)?;
    let range = (
        optional_number(entry, path, "min")?,
        optional_number(entry, path, "max")?,
    );
    if yes_no && range != (None, None) {
        let key = if range.0.is_some() { "min" } else { "max" };
        return fail(
            &at(path, key),
            "a yes/no test filters on its answer, not a range",
        );
    }
    let score = score_of(entry, path)?;
    let (filters, scores) = purpose(entry, path, yes_no, range, score.is_some())?;
    let filter = match (filters, yes_no, range) {
        (false, ..) => None,
        (true, true, _) => Some(Filter::Wanted),
        (true, false, (None, None)) => return fail(path, "a filter on a number needs min or max"),
        (true, false, (min, max)) => Some(Filter::Range { min, max }),
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
/// with no score filters, `min`/`max` filter, a score scores. An explicit
/// purpose that a field contradicts (`"score"` with `min`/`max`, `"filter"`
/// with a `score`) is an error naming that field.
fn purpose(
    entry: &Table,
    path: &str,
    yes_no: bool,
    range: (Option<f32>, Option<f32>),
    has_score: bool,
) -> mlua::Result<(bool, bool)> {
    let said = text(entry, path, "purpose")?;
    let (filters, scores) = match said.as_deref() {
        None => return Ok((range != (None, None) || (yes_no && !has_score), has_score)),
        Some("filter") => (true, false),
        Some("score") => (false, true),
        Some("filter_score") => (true, true),
        Some(other) => {
            return fail(
                &at(path, "purpose"),
                format!("unknown purpose '{other}' (expected filter, score, filter_score)"),
            )
        }
    };
    let said = said.unwrap_or_default();
    if !filters && range != (None, None) {
        let key = if range.0.is_some() { "min" } else { "max" };
        return fail(
            &at(path, key),
            format!("purpose '{said}' does not filter, so {key} would be ignored"),
        );
    }
    if !scores && has_score {
        return fail(
            &at(path, "score"),
            format!("purpose '{said}' does not score, so score would be ignored"),
        );
    }
    Ok((filters, scores))
}

/// The measure `kind` names, and whether it is a yes/no test.
fn measure(
    entry: &Table,
    kind: &str,
    path: &str,
    contexts: &Contexts,
) -> mlua::Result<(Measure, bool)> {
    if kind == "path" {
        return match text(entry, "", "path")
            .or_else(|_| fail(path, "expected \"exists\" or \"length\""))?
        {
            Some(m) if m == "exists" => Ok((Measure::PathExists, true)),
            Some(m) if m == "length" => Ok((Measure::PathLength, false)),
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

use super::{EffectKind, EffectParams, KindId, MAX_PARAMS};
use crate::mixer::GroupId;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64),
    Text(String),
    List(Vec<Fields>),
}

pub type Fields = Vec<(String, Value)>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Range {
    /// A number in `[min, max]`, glided linearly.
    Number { min: f32, max: f32, default: f32 },
    /// A frequency in Hz in `[min, max]`, glided in octaves.
    Hz { min: f32, max: f32, default: f32 },
    /// One of `names` (stored as its index), switched at once.
    Choice {
        names: &'static [&'static str],
        default: u8,
    },
    /// Another mixer group, by name (stored as its index; `-1` is none).
    Group,
}

impl Range {
    /// The value a parameter not named starts at.
    pub fn default(&self) -> f32 {
        match *self {
            Range::Number { default, .. } | Range::Hz { default, .. } => default,
            Range::Choice { default, .. } => f32::from(default),
            Range::Group => -1.0,
        }
    }

    pub fn lerp(&self, a: f32, b: f32, t: f32) -> f32 {
        match self {
            _ if a == b || t >= 1.0 => b,
            Range::Number { .. } => a + (b - a) * t,
            Range::Hz { .. } => {
                let (la, lb) = (a.max(1.0).log2(), b.max(1.0).log2());
                (la + (lb - la) * t).exp2()
            }
            Range::Choice { .. } | Range::Group => b,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Param {
    pub name: &'static str,
    pub range: Range,
}

pub type Resolve<'a> = dyn Fn(&str) -> Result<Option<GroupId>, String> + 'a;

pub fn group(v: f32) -> Option<GroupId> {
    (v >= 0.0).then_some(GroupId(v as u16))
}

pub fn parse(
    kind: KindId,
    fields: &Fields,
    base: &EffectParams,
    resolve: &Resolve,
) -> Result<(EffectParams, u32), String> {
    let row = kind.row();
    let mut out = *base;
    let mut mask = 0u32;
    for (name, value) in flatten(row, fields) {
        if name == "kind" {
            continue;
        }
        let i = row.params.iter().position(|p| p.name == name);
        let Some(i) = i else {
            let names: Vec<_> = row.params.iter().map(|p| p.name).collect();
            return Err(format!(
                "{} has no parameter '{name}' (parameters: {})",
                row.name,
                names.join(", ")
            ));
        };
        out.values[i] = check(row, &row.params[i], &value, resolve)?;
        mask |= 1 << i;
    }
    Ok((out, mask))
}

fn flatten(row: &EffectKind, fields: &Fields) -> Fields {
    let mut out = Fields::new();
    for (name, value) in fields {
        match (value, row.list) {
            (Value::List(entries), Some(list)) if name == list => {
                for (n, entry) in entries.iter().enumerate() {
                    for (field, v) in entry {
                        out.push((format!("{list}{}_{field}", n + 1), v.clone()));
                    }
                }
            }
            _ => out.push((name.clone(), value.clone())),
        }
    }
    out
}

fn check(row: &EffectKind, p: &Param, value: &Value, resolve: &Resolve) -> Result<f32, String> {
    let at = format!("{}.{}", row.name, p.name);
    match (p.range, value) {
        (Range::Number { min, max, .. } | Range::Hz { min, max, .. }, Value::Number(n)) => {
            let v = *n as f32;
            if v.is_finite() && (min..=max).contains(&v) {
                Ok(v)
            } else {
                Err(format!("{at} = {n} is out of range [{min}, {max}]"))
            }
        }
        (Range::Choice { names, .. }, Value::Text(s)) => names
            .iter()
            .position(|n| n == s)
            .map(|i| i as f32)
            .ok_or_else(|| format!("{at} = '{s}' is not one of: {}", names.join(", "))),
        (Range::Group, Value::Text(s)) => Ok(resolve(s)?.map_or(-1.0, |g| f32::from(g.0))),
        (range, _) => Err(format!("{at} takes {}", expects(range))),
    }
}

fn expects(range: Range) -> &'static str {
    match range {
        Range::Number { .. } | Range::Hz { .. } => "a number",
        Range::Choice { .. } => "a name",
        Range::Group => "a group name",
    }
}

pub fn emit(params: &EffectParams, name: &dyn Fn(GroupId) -> String) -> Fields {
    let row = params.kind.row();
    let mut out = vec![("kind".to_string(), Value::Text(row.name.to_string()))];
    let mut entries: Vec<Fields> = Vec::new();
    for (p, v) in row.params.iter().zip(params.values).take(MAX_PARAMS) {
        let value = match p.range {
            Range::Number { .. } | Range::Hz { .. } => Value::Number(f64::from(v)),
            Range::Choice { names, .. } => {
                Value::Text(names.get(v as usize).copied().unwrap_or("").to_string())
            }
            Range::Group => Value::Text(group(v).map(name).unwrap_or_default()),
        };
        match row.list.and_then(|l| list_entry(p.name, l)) {
            Some((n, field)) => {
                entries.resize_with(entries.len().max(n), Fields::new);
                entries[n - 1].push((field.to_string(), value));
            }
            None => out.push((p.name.to_string(), value)),
        }
    }
    if let Some(list) = row.list {
        out.push((list.to_string(), Value::List(entries)));
    }
    out
}

fn list_entry<'a>(name: &'a str, list: &str) -> Option<(usize, &'a str)> {
    let (n, field) = name.strip_prefix(list)?.split_once('_')?;
    Some((n.parse().ok().filter(|n| *n > 0)?, field))
}

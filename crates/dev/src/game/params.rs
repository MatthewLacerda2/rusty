#[derive(Clone, Debug, Default, PartialEq)]
pub struct Params(Vec<(String, Value)>);

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Number(f64),
    Word(String),
}

impl Params {
    pub fn parse<'a>(pairs: impl IntoIterator<Item = &'a str>) -> Result<Self, String> {
        let mut params = Vec::new();
        for pair in pairs {
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| format!("--param {pair}: expected key=value"))?;
            if !is_identifier(key) {
                return Err(format!("--param {pair}: '{key}' is not a Lua name"));
            }
            params.push((key.to_string(), value_of(pair, value)?));
        }
        Ok(Self(params))
    }

    pub fn to_lua(&self) -> String {
        let fields: Vec<String> = self
            .0
            .iter()
            .map(|(key, value)| match value {
                Value::Bool(b) => format!("{key} = {b}"),
                Value::Number(n) => format!("{key} = {}", lua_number(*n)),
                Value::Word(w) => format!("{key} = \"{w}\""),
            })
            .collect();
        match fields.is_empty() {
            true => "{}".to_string(),
            false => format!("{{ {} }}", fields.join(", ")),
        }
    }
}

fn lua_number(n: f64) -> String {
    const WHOLE: f64 = 9_007_199_254_740_992.0;
    match n.fract() == 0.0 && n.abs() < WHOLE {
        true => format!("{}", n as i64),
        false => format!("{n:?}"),
    }
}

fn value_of(pair: &str, value: &str) -> Result<Value, String> {
    match value {
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        _ => {}
    }
    if let Some(n) = value.parse::<f64>().ok().filter(|n| n.is_finite()) {
        return Ok(Value::Number(n));
    }
    let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.');
    if value.is_empty() || !value.chars().all(plain) {
        return Err(format!(
            "--param {pair}: a value is true, false, a number or a word of letters, digits, _ - ."
        ));
    }
    Ok(Value::Word(value.to_string()))
}

fn is_identifier(key: &str) -> bool {
    let mut chars = key.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;

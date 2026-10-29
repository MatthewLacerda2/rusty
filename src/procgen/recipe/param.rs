//! src/procgen/recipe/param.rs — a socketable param: a number, or a link to another
//! node (#1494).
//!
//! Blender lets any input socket take another node, so a value varies across the
//! map. rusty does that for a curated set of continuous, amount-like params, the
//! ones the catalog marks `socket` ([`super::OpParam::socket`]): a warp's strength,
//! a blur's radius, a tile's jitter. A socketable param reads either form:
//!
//! ```lua
//! strength = 0.1                                       -- one value for the map
//! strength = { node = "damage", min = 0, max = 0.2 }   -- min + (max − min) × damage.red
//! ```
//!
//! A number serializes as the bare number it always was, so an all-number recipe
//! writes, and bakes, exactly as before. A link reads its node's red channel, like
//! every mask input; `separate_rgb` picks another channel.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// A socketable param's value: one number for the whole map, or a [`Link`].
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Param {
    Num(f32),
    Link(Link),
}

/// A param driven by node `node`: `min + (max − min) × red` per pixel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub node: String,
    #[serde(default)]
    pub min: f32,
    #[serde(default = "one")]
    pub max: f32,
}

impl Param {
    /// The link, when the param is one.
    pub fn link(&self) -> Option<&Link> {
        match self {
            Param::Num(_) => None,
            Param::Link(l) => Some(l),
        }
    }
}

impl Default for Param {
    fn default() -> Self {
        Param::Num(0.0)
    }
}

impl From<f32> for Param {
    fn from(v: f32) -> Self {
        Param::Num(v)
    }
}

/// Parsed by hand rather than `untagged`, whose error names neither form: a bad
/// link says which of its keys is wrong.
impl<'de> Deserialize<'de> for Param {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match Value::deserialize(d)? {
            Value::Number(n) => Ok(Param::Num(n.as_f64().unwrap_or(f64::NAN) as f32)),
            link @ Value::Object(_) => Link::deserialize(link)
                .map(Param::Link)
                .map_err(|e| D::Error::custom(format!("bad link: {e}"))),
            other => Err(D::Error::custom(format!(
                "expected a number or a link {{ node, min, max }}, got {other}"
            ))),
        }
    }
}

fn one() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Result<Param, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    #[test]
    fn a_number_and_a_link_both_parse_and_round_trip() {
        assert_eq!(parse("0.25"), Ok(Param::Num(0.25)));
        assert_eq!(parse("3"), Ok(Param::Num(3.0)));
        let link = parse(r#"{ "node": "m" }"#).unwrap();
        let Param::Link(l) = &link else { panic!() };
        assert_eq!((l.node.as_str(), l.min, l.max), ("m", 0.0, 1.0));
        let back = serde_json::to_string(&link).unwrap();
        assert_eq!(parse(&back), Ok(link));
        assert_eq!(serde_json::to_string(&Param::Num(0.5)).unwrap(), "0.5");
    }

    #[test]
    fn a_misspelled_link_key_or_another_type_is_refused() {
        let err = parse(r#"{ "node": "m", "mx": 2 }"#).unwrap_err();
        assert!(err.contains("bad link") && err.contains("mx"), "{err}");
        let err = parse(r#""m""#).unwrap_err();
        assert!(err.contains("a number or a link"), "{err}");
    }
}

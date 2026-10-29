//! Writing a partial set of quality values over a full one (#1522): how a preset in
//! `project.rusty` that names only some values loads (the rest are the seeded
//! `Medium` preset's, so a partial file keeps the defaults for what it omits, as
//! the rest of `ProjectFile` does), and how `Graphics.SetQualityPreset` edits one.

use serde::de::{DeserializeOwned, Error};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::{seeds, Levels, SsaoQuality, SsrQuality};

/// Write `patch` over `base` key by key, `path` naming where for a refusal. A key
/// `base` doesn't have is refused, so a misspelled value never loads silently.
pub(super) fn merge(base: &mut Value, patch: &Value, path: &str) -> Result<(), String> {
    let (Value::Object(base), Value::Object(patch)) = (&mut *base, patch) else {
        *base = patch.clone();
        return Ok(());
    };
    for (key, value) in patch {
        let at = format!("{path}{key}");
        let slot = base
            .get_mut(key)
            .ok_or_else(|| format!("a quality preset has no '{at}'"))?;
        merge(slot, value, &format!("{at}."))?;
    }
    Ok(())
}

/// Read one effect's levels as `patch` over `base`, the seeded `Medium` preset's.
fn over<'de, D, T>(d: D, base: T, path: &str) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Serialize + DeserializeOwned,
{
    let patch = Value::deserialize(d)?;
    let mut merged = serde_json::to_value(base).map_err(D::Error::custom)?;
    merge(&mut merged, &patch, path).map_err(D::Error::custom)?;
    serde_json::from_value(merged).map_err(D::Error::custom)
}

pub(super) fn ssao<'de, D: Deserializer<'de>>(d: D) -> Result<Levels<SsaoQuality>, D::Error> {
    over(d, seeds::medium().ssao, "ssao.")
}

pub(super) fn ssr<'de, D: Deserializer<'de>>(d: D) -> Result<Levels<SsrQuality>, D::Error> {
    over(d, seeds::medium().ssr, "ssr.")
}

pub(super) fn bloom<'de, D: Deserializer<'de>>(d: D) -> Result<Levels<u32>, D::Error> {
    let base = seeds::medium().bloom_resolution_divisor;
    over(d, base, "bloom_resolution_divisor.")
}

pub(super) fn depth_of_field<'de, D: Deserializer<'de>>(d: D) -> Result<Levels<u32>, D::Error> {
    let base = seeds::medium().depth_of_field_resolution_divisor;
    over(d, base, "depth_of_field_resolution_divisor.")
}

pub(super) fn motion_blur<'de, D: Deserializer<'de>>(d: D) -> Result<Levels<u32>, D::Error> {
    let base = seeds::medium().motion_blur_samples;
    over(d, base, "motion_blur_samples.")
}

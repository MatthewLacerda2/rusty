//! A material's shader inputs from Lua: runtime params and per-entity overrides
//! (`params`, #399, #670) and the extra texture slots (`textures`, #400).

pub(crate) mod params;
pub(super) mod textures;

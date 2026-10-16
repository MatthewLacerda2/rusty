//! Light-probe baking: capture + SH projection (`bake`, #241) and the multi-bounce
//! solve on top of it (`bounce`, #285).

pub(crate) mod bake;
pub(crate) mod bounce;

//! Light-probe baking: capture + SH projection (`bake`, #241) and the multi-bounce
//! solve on top of it (`bounce`, #285), stepped one probe at a time (`solve`, #1602).
//! A light probe's capture sees the ambient where it escapes the scene, not the
//! sky (`capture`, #1668).

pub(crate) mod bake;
pub mod bounce;
pub(crate) mod capture;
pub mod solve;

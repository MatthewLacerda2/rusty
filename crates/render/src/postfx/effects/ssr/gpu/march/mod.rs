//! The march's GPU tests (#2228, `lighting/ssr_march.wgsl`): what it finds of a
//! thin line under TAA's jitter, where it places a hit, and how deep it takes
//! what it hits.

mod hits;
mod placing;
mod thickness;
mod thin;

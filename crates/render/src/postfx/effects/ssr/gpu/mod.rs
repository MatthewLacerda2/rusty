//! SSR on a GPU (#1526, #1542): the shared scene, what reflects and what doesn't,
//! what a hit replaces, and what draws over it.

mod blur;
mod cover;
mod energy;
mod fixture;
mod ops;
mod packing;
mod reach;
mod reflect;
mod rough;
mod silhouette;
mod transparent;

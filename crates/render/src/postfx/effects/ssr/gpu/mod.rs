//! SSR on a GPU (#1526, #1542): the shared scene, what reflects and what doesn't,
//! what a hit replaces, and what draws over it.

mod blur;
mod cover;
mod edge;
mod energy;
mod fixture;
mod glossy;
mod packing;
mod reach;
mod reflect;
mod resolve;
mod silhouette;
mod transparent;

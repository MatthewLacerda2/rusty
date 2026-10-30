//! SSR on a GPU (#1526, #1542): the shared scene, what reflects and what doesn't,
//! what a hit replaces, and what draws over it.

mod blur;
mod cover;
mod energy;
mod fixture;
mod reflect;
mod transparent;

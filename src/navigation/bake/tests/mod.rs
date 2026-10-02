//! Tests for the layered bake (#454): the rasterised surface, stacked floors, the
//! headroom filter (#278) and the agent-radius erosion (#277); and for the
//! incremental rebake and obstacle carving (#456); and for the area modifier
//! volumes (#460). Every scene uses real box
//! colliders (transform + shape) and an explicit floor: nothing is walkable where
//! no geometry is.

mod areas;
mod carving;
mod doorway;
mod erosion;
mod headroom;
mod incremental;
mod layers;
mod settings;
mod surface;

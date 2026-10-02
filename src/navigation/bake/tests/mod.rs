//! Tests for the layered bake (#454): the rasterised surface, stacked floors, the
//! headroom filter (#278) and the agent-radius erosion (#277). Every scene uses real
//! box colliders (transform + shape) and an explicit floor: nothing is walkable
//! where no geometry is.

mod erosion;
mod headroom;
mod layers;
mod settings;
mod surface;

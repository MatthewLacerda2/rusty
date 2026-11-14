//! The navigation inspector cards: NavMesh Agent, Modifier Volume, Modifier and
//! Obstacle.

mod agent;
mod modifier;
mod object_modifier;
mod obstacle;

pub use agent::draw_nav_agent;
pub use modifier::draw_nav_modifier;
pub use object_modifier::draw_nav_mesh_modifier;
pub use obstacle::draw_nav_obstacle;

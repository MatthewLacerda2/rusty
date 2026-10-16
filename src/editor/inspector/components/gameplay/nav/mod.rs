//! The navigation inspector cards: NavMesh Agent, Modifier Volume and Obstacle.

mod agent;
mod modifier;
mod obstacle;

pub use agent::draw_nav_agent;
pub use modifier::draw_nav_modifier;
pub use obstacle::draw_nav_obstacle;

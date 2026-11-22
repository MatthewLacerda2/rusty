//! The scene's half of the play-mode requests a script only queues (destroys, loads), drained
//! by the tick, and the play state. The swap itself is `rusty_edit::load`'s.

pub mod debug_lines;
pub mod despawn;
pub mod destroy_queue;
pub mod load;
pub mod loads;
pub mod renumber;

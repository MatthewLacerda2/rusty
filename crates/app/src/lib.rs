#![deny(clippy::unwrap_used)]

pub mod animation;
mod audio;
mod edit_runtime;
pub mod game;
mod play;
mod probe;
mod registry;
mod resources;
mod scene_load;
mod schedule;
mod stage;
mod systems;
mod tap;
mod world;

pub use game::{GameWorld, PlayTransition};
pub use probe::{timed, LoadProbe, SystemProbe};
pub use registry::{build, App};
pub use resources::Resources;
/// Test hook: the fixture project every entry point's test opens.
#[doc(hidden)]
pub use scene_load::project_fixture;
pub use scene_load::{
    load_boot_scene, prepare_counted, prepare_in_background, Inline, PrepareSteps, ProjectSource,
    SceneLoader, ABANDONED,
};
pub use schedule::Schedule;
pub use stage::Stage;
pub use tap::{TapPoint, TickTap};
pub use world::World;

#[cfg(test)]
mod late_update_tests;
#[cfg(test)]
mod lifecycle_tests;
#[cfg(test)]
mod missing_bone_tests;
#[cfg(test)]
mod model_writes_tests;
#[cfg(test)]
mod probe_tests;
#[cfg(test)]
mod schedule_tests;

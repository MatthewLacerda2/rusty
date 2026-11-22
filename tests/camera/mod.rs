//! Integration coverage for the `Camera` namespace: the main view's one pose
//! (`main_view`) and its same-frame readers (`same_frame`), the per-entity
//! lens and stacking functions (`stack_api`), render-texture targets
//! (`target_api`), history cuts (`reset_history`) and the FOV axis
//! and its conversions (`fov_axis`).

mod fov_axis;
mod main_view;
mod reset_history;
mod same_frame;
mod stack_api;
mod target_api;

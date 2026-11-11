//! Integration coverage for the `Camera` namespace: the main view's one pose
//! (`main_view`, #1619) and its same-frame readers (`same_frame`), the per-entity
//! lens and stacking functions (`stack_api`, #1341), render-texture targets
//! (`target_api`, #430) and history cuts (`reset_history`, #2003).

mod fov_axis;
mod main_view;
mod reset_history;
mod same_frame;
mod stack_api;
mod target_api;

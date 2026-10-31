//! Integration coverage for the `Camera` namespace: the main view's one pose
//! (`main_view`, #1619) and its same-frame readers (`same_frame`), the per-entity
//! lens and stacking functions (`stack_api`, #1341) and render-texture targets
//! (`target_api`, #430).

mod main_view;
mod same_frame;
mod stack_api;
mod target_api;

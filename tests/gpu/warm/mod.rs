//! A load's GPU warm-up (`render::warm`) over the load path itself: a scene
//! saved, prepared off the frame as the background load prepares it, warmed, and
//! swapped in. What the plan lists is `render`'s own test (`warm::plan_tests`).

mod first_frame;
mod prefab;

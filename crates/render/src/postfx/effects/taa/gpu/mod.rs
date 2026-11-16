//! TAA's GPU tests (#1702, #1846), on lavapipe: what they share (`fixture`,
//! `wall`'s noise wall and `still`'s switch for #1846's rules), then one file
//! per behaviour (`reactive`: #1845's moving translucent; `knobs`: #1847's camera
//! settings; `flicker`: #2228's anti-flicker over a jittered SSR outlier). TAAU's
//! are in `upscale/`.

mod bob;
mod cut;
pub(super) mod fixture;
pub(in crate::postfx::effects) mod flicker;
mod ghost;
mod knobs;
mod reactive;
mod resolve;
mod reveal;
mod shimmer;
pub(super) mod still;
mod turn;
mod viewmodel;
mod wall;

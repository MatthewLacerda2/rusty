//! TAA's GPU tests (#1702, #1846), on lavapipe: what they share (`fixture`, and
//! `wall`'s noise wall), then one file per behaviour. TAAU's are in
//! `upscale/`.

mod cut;
pub(super) mod fixture;
mod ghost;
mod resolve;
mod reveal;
mod shimmer;
mod turn;
mod viewmodel;
mod wall;

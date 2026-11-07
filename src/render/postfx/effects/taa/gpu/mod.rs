//! TAA's GPU tests (#1702, #1846), on lavapipe: what they share (`fixture`,
//! `wall`'s noise wall and `still`'s switch for #1846's rules), then one file per
//! behaviour (`reactive`: #1845's moving translucent). TAAU's are in
//! `upscale/`.

mod bob;
mod cut;
pub(super) mod fixture;
mod ghost;
mod reactive;
mod resolve;
mod reveal;
mod shimmer;
pub(super) mod still;
mod turn;
mod viewmodel;
mod wall;

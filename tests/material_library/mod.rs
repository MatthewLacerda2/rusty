//! Material library (#201): materials are reusable assets in the scene's library,
//! entities reference one by name. Covers sharing, round-trip, and back-compat with
//! pre-#201 scenes that stored the material inline as `texture`.

mod api_parity;
mod legacy;
mod library;

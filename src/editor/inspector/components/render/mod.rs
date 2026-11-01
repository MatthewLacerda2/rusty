//! src/editor/inspector/components/render/ — the rendering cards that light a mesh:
//! the Light card (`light.rs`) and the Light Probe Proxy Volume card
//! (`light_probe_proxy_volume.rs`, #1672).

mod light;
mod light_probe_proxy_volume;

pub use light::draw_light;
pub use light_probe_proxy_volume::draw_light_probe_proxy_volume;

//! src/render/render_texture/ — render textures (#430): cameras that draw into a
//! named texture instead of the screen, shown by UI `Image`s and material maps as
//! `"rt:<name>"` (Unity's `Camera.targetTexture` + `RawImage`).
//!
//! **Frame order.** `Renderer::render` syncs the view's textures (allocating,
//! resizing, dropping) and registers each in the by-path texture cache, so every
//! consumer resolves `"rt:<name>"` through the path it already uses for files. The
//! shadow cascades are fitted once, to the screen's base camera, and then each
//! render-texture camera draws its own single-camera stack before the screen does —
//! so the screen samples this frame's picture.
//!
//! **Shadows.** A render-texture camera samples the screen camera's cascades rather
//! than fitting its own: one shadow pass per frame, and the places a minimap, scope
//! or preview looks at are near the player, where those cascades already are.
//! Beyond them its view is unshadowed, as the screen's is past `shadow_distance`.
//!
//! **Cost.** A camera draws only when a texture it targets is *referenced* by an
//! active Image or material this frame, and at most every `update_every` frames;
//! `RenderCounters::render_texture_draws` counts the draws, and their geometry is in
//! the frame's `draw_calls` / `triangles` like any camera's.

mod slots;

use std::collections::HashSet;
use std::rc::Rc;

pub use slots::RenderTextures;

use crate::components::RENDER_TEXTURE_PREFIX;
use crate::render::draw::stack::StackPass;
use crate::render::{RenderView, Renderer, TextureCamera};
use crate::scene::Scene;

/// Whether `path` names a render texture rather than a file.
pub(crate) fn is_render_texture(path: &str) -> bool {
    path.starts_with(RENDER_TEXTURE_PREFIX)
}

impl Renderer {
    /// Make `view`'s render textures match `cameras`' targets — allocate new ones,
    /// reallocate resized ones, drop untargeted ones — and register exactly them in
    /// the by-path texture cache (another view's are unregistered, so a scene never
    /// shows a texture it has no camera for).
    pub(crate) fn sync_render_textures(
        &mut self,
        view: &mut RenderView,
        cameras: &[TextureCamera],
    ) {
        let rts = &mut view.render_textures;
        rts.frame += 1;
        let wanted: HashSet<String> = cameras.iter().map(|c| c.target.path()).collect();
        rts.slots.retain(|path, _| wanted.contains(path));
        for cam in cameras {
            let size = (cam.target.width.max(1), cam.target.height.max(1));
            let path = cam.target.path();
            if rts.slots.get(&path).map(|s| s.size) != Some(size) {
                let slot = self.new_render_texture(size.0, size.1);
                rts.slots.insert(path, slot);
            }
        }
        let bloom = self.quality.bloom_divisor();
        self.gpu_textures.retain(|path, _| !is_render_texture(path));
        self.render_texture_ids.0.clear();
        for (path, slot) in &mut rts.slots {
            let (w, h) = slot.size;
            slot.view.resize(&self.device, w, h, bloom);
            self.gpu_textures
                .insert(path.clone(), Rc::clone(&slot.front));
            self.render_texture_ids.0.insert(path.clone(), slot.id);
        }
    }

    /// Draw every due, referenced render-texture camera into its texture (see the
    /// module docs). Runs after the shadow passes and before the screen stack.
    pub(crate) fn draw_render_textures(
        &mut self,
        view: &mut RenderView,
        scene: &Scene,
        cameras: &[TextureCamera],
    ) {
        if cameras.is_empty() {
            return;
        }
        let shown = referenced_render_textures(scene);
        let frame = view.render_textures.frame;
        for cam in cameras {
            let path = cam.target.path();
            let Some(slot) = view.render_textures.slots.get_mut(&path) else {
                continue;
            };
            if !shown.contains(&path) || !slot.is_due(frame, cam.target.update_every) {
                continue;
            }
            let Some(target) = slot.view.color_target_view() else {
                continue;
            };
            let pass = StackPass {
                stack: std::slice::from_ref(&cam.camera),
                output: &target,
                editor_mode: false,
                full_post_fx: cam.target.post_fx,
            };
            self.draw_stack(&mut slot.view, scene, pass);
            self.copy_to_front(&slot.view, &slot.front.texture);
            slot.drawn_on = Some(frame);
            self.frame_counters.render_texture_draws += 1;
        }
    }

    /// Copy a render-texture view's finished target into the texture consumers sample.
    fn copy_to_front(&self, view: &RenderView, front: &wgpu::Texture) {
        let Some(source) = view.color_target() else {
            return;
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Texture Copy"),
            });
        encoder.copy_texture_to_texture(
            source.as_image_copy(),
            front.as_image_copy(),
            front.size(),
        );
        self.queue.submit(Some(encoder.finish()));
    }

    /// The material-cache key of a render texture's path: the path plus its current
    /// allocation, so a resized texture gets a fresh bind group. `None` for a file.
    pub(crate) fn render_texture_key(&self, path: &str) -> Option<String> {
        let id = self.render_texture_ids.0.get(path)?;
        Some(format!("{path}#{id}"))
    }
}

/// Every `"rt:<name>"` path an active UI `Image` or an active entity's material
/// maps reference this frame — the textures worth drawing.
pub(crate) fn referenced_render_textures(scene: &Scene) -> HashSet<String> {
    let images = scene
        .world
        .ids_with_image()
        .into_iter()
        .filter(|&id| scene.world.is_active(id))
        .filter_map(|id| Some(scene.world.image(id)?.shown_texture()?.to_string()));
    let maps = crate::render::draw::active_material_map_paths(scene).into_iter();
    images
        .chain(maps)
        .filter(|path| is_render_texture(path))
        .collect()
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

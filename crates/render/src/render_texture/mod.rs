mod slots;

use crate::gpu::commands::Commands;
use std::collections::HashSet;
use std::rc::Rc;

use slots::frames_since;
pub use slots::RenderTextures;

use crate::draw::stack::StackPass;
use crate::postfx::params::borrow_post_source;
use crate::{RenderView, Renderer, TextureCamera};
use rusty_asset::MapRef;
pub(crate) use rusty_components::is_render_texture;
use rusty_scene::Scene;

impl Renderer {
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
        self.gpu_textures
            .retain(|path, _| !is_render_texture(path.as_str()));
        let before = std::mem::take(&mut self.render_texture_ids.0);
        for (path, slot) in &mut rts.slots {
            let (w, h) = slot.size;
            slot.view.resize(&self.device, w, h);
            self.gpu_textures
                .insert(MapRef::new(path.clone()), Rc::clone(&slot.front));
            self.render_texture_ids.0.insert(path.clone(), slot.id);
        }
        if self.render_texture_ids.0 != before {
            self.retained.render_textures += 1;
        }
    }

    pub(crate) fn draw_render_textures(
        &mut self,
        view: &mut RenderView,
        scene: &Scene,
        cameras: &[TextureCamera],
        mut commands: Commands,
    ) -> Commands {
        if cameras.is_empty() {
            return commands;
        }
        let shown = referenced_render_textures(scene);
        let frame = view.render_textures.frame;
        for cam in cameras {
            let path = cam.target.path();
            let Some(slot) = view.render_textures.slots.get_mut(&path) else {
                continue;
            };
            if !shown.contains(&path) {
                continue;
            }
            self.frame_counters.history_passes += 1;
            if !slot.is_due(frame, cam.target.update_every) {
                continue;
            }
            let Some(target) = slot.view.color_target_view() else {
                continue;
            };
            let motion_frames = frames_since(slot.drawn_on, frame);
            let guards = borrow_post_source(scene, Some(cam.id));
            let pass = StackPass {
                stack: std::slice::from_ref(&cam.camera),
                entities: &[Some(cam.id)],
                base_id: Some(cam.id),
                post: guards.source(),
                output: &target,
                editor_mode: false,
                full_post_fx: cam.target.post_fx,
                motion_frames,
            };
            let mut last = self.draw_stack(&mut slot.view, scene, (pass, commands));
            copy_to_front(last.encoder(), &slot.view, &slot.front.texture);
            // The next camera rewrites the camera uniform: this one's draws go out first.
            last.flush();
            commands = last;
            slot.drawn_on = Some(frame);
            self.frame_counters.render_texture_draws += 1;
        }
        commands
    }

    pub(crate) fn render_texture_key(&self, path: &str) -> Option<String> {
        let id = self.render_texture_ids.0.get(path)?;
        Some(format!("{path}#{id}"))
    }
}

pub(crate) fn referenced_render_textures(scene: &Scene) -> HashSet<String> {
    let images = scene
        .world
        .ids_with_image()
        .into_iter()
        .filter(|&id| scene.world.is_active_in_hierarchy(id))
        .filter_map(|id| scene.world.image(id)?.shown_texture().cloned());
    let maps = crate::draw::material_map_paths(scene, |id| scene.world.is_active_in_hierarchy(id))
        .into_iter()
        .map(|(path, _)| path);
    let dirt = crate::postfx::effects::bloom::dirt::render_textures(scene);
    images
        .chain(maps)
        .chain(dirt.into_iter().map(Into::into))
        .filter(|path| is_render_texture(path.as_str()))
        .map(|path| path.to_string())
        .collect()
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

fn copy_to_front(encoder: &mut wgpu::CommandEncoder, view: &RenderView, front: &wgpu::Texture) {
    let Some(source) = view.color_target() else {
        return;
    };
    let to = front.as_image_copy();
    encoder.copy_texture_to_texture(source.as_image_copy(), to, front.size());
}

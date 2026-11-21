use std::collections::BTreeSet;
use std::sync::{Arc, Weak};

use super::Residency;
use crate::draw::map_slots::material_maps;
use crate::render_texture::is_render_texture;
use crate::{MeshId, Renderer};
use rusty_asset::MapRef;
use rusty_scene::{Scene, SceneId};

pub(super) struct Warmed {
    pub token: Weak<()>,
    pub meshes: BTreeSet<MeshId>,
    pub maps: BTreeSet<MapRef>,
    pub lightmaps: Vec<Vec<String>>,
}

impl Warmed {
    pub(super) fn new(token: &Arc<()>) -> Self {
        Self {
            token: Arc::downgrade(token),
            meshes: BTreeSet::new(),
            maps: BTreeSet::new(),
            lightmaps: Vec::new(),
        }
    }
}

impl Residency {
    pub(crate) fn warmed_map(&mut self, token: &Arc<()>, map: &MapRef) {
        self.warmup(token).maps.insert(map.clone());
    }

    pub(crate) fn warmed_lightmaps(&mut self, token: &Arc<()>, lists: [&[String]; 3]) {
        let warmed = &mut self.warmup(token).lightmaps;
        for list in lists.into_iter().filter(|l| !l.is_empty()) {
            if !warmed.iter().any(|w| w == list) {
                warmed.push(list.to_vec());
            }
        }
    }
}

impl Renderer {
    pub(crate) fn note_map(&mut self, id: SceneId, map: &str) {
        super::notes::counted();
        let maps = self.retained.residency.maps.entry(id).or_default();
        if !maps.contains(map) {
            maps.insert(MapRef::from(map));
        }
    }

    pub(super) fn note_held_maps(&mut self, scene: &Scene) {
        let maps: Vec<MapRef> = scene_maps(scene).collect();
        for map in maps {
            self.note_map(scene.id(), map.as_str());
        }
    }

    pub(super) fn note_lightmaps(&mut self, id: SceneId, lists: [&[String]; 3]) {
        super::notes::counted();
        let noted = &mut self.retained.residency.lightmaps;
        let same = |kept: &[Vec<String>; 3]| kept.iter().zip(lists).all(|(k, l)| k == l);
        if !noted.get(&id).is_some_and(same) {
            noted.insert(id, lists.map(|l| l.to_vec()));
        }
    }

    pub(crate) fn release_unused_textures(&mut self, scene: &Scene) -> usize {
        let used = self.used_maps(scene);
        let kept = |map: &MapRef| is_render_texture(map.as_str()) || used.contains(map.as_str());
        self.texture_fallbacks.retain(|map, _| kept(map));
        let gone: BTreeSet<String> = (self.gpu_textures.keys())
            .filter(|map| !kept(map))
            .map(|map| map.as_str().to_owned())
            .collect();
        if gone.is_empty() {
            return 0;
        }
        self.gpu_textures.retain(|map, _| kept(map));
        let is_gone = |key: &str| gone.contains(key);
        self.materials.forget_groups_binding(is_gone);
        self.ui_renderer.forget_textures(is_gone);
        gone.len()
    }

    fn used_maps(&mut self, scene: &Scene) -> BTreeSet<String> {
        let retained = &mut self.retained;
        let objects = retained.scenes.iter().flat_map(|(_, s)| s.objects.values());
        let groups: BTreeSet<usize> = objects
            .flat_map(|o| o.slots.iter().map(|slot| slot.key.material))
            .collect();
        let mut used = self.materials.maps_of(&groups);
        let noted = retained.residency.maps.values().flatten();
        used.extend(noted.map(|m| m.as_str().to_owned()));
        let warming = retained.residency.warming().iter();
        used.extend(warming.flat_map(|w| &w.maps).map(|m| m.as_str().to_owned()));
        used.extend(scene_maps(scene).map(|m| m.as_str().to_owned()));
        used.insert(self.skybox_path.clone());
        used
    }

    pub(crate) fn release_unused_lightmaps(&mut self) {
        let residency = &mut self.retained.residency;
        let mut used: BTreeSet<Vec<String>> = (residency.warming().iter())
            .flat_map(|w| w.lightmaps.iter().cloned())
            .collect();
        used.extend(residency.lightmaps.values().flatten().cloned());
        self.lightmaps.release(|list| used.contains(list));
    }
}

pub(crate) fn scene_maps(scene: &Scene) -> impl Iterator<Item = MapRef> + '_ {
    let world = &scene.world;
    let images = (world.ids_with_image().into_iter()).filter_map(|id| world.image(id));
    let images = images.flat_map(|i| [i.texture.clone(), i.override_texture.clone()]);
    let selectables = (world.ids_with_selectable().into_iter())
        .filter_map(|id| world.selectable(id))
        .flat_map(|s| s.sprites.clone());
    let emitters = world.ids_with_particles().into_iter();
    let emitters = emitters.filter_map(|id| world.particles(id)).flat_map(|e| {
        let material = (e.render.material.as_ref()).and_then(|m| scene.materials.get(m));
        let maps = material_maps(material).map(|(map, _)| Some(map));
        std::iter::once(e.texture.clone())
            .chain(maps)
            .collect::<Vec<_>>()
    });
    let trails = (world.ids_with_trail().into_iter()).filter_map(|id| world.trail(id));
    let trails = trails.map(|t| t.style.texture.clone());
    let lines = (world.ids_with_line().into_iter()).filter_map(|id| world.line(id));
    let lines = lines.map(|l| l.style.texture.clone());
    let sky = (!scene.skybox_path.is_empty()).then(|| MapRef::from(scene.skybox_path.as_str()));
    (images
        .chain(selectables)
        .chain(emitters)
        .chain(trails)
        .chain(lines))
    .flatten()
    .chain(sky)
}

use std::collections::HashMap;

use super::record::{self, DecalMaps, GpuDecal};
use rusty_asset::MapRef;
use rusty_scene::decal::Decal;
use rusty_scene::revision::Revision;
use rusty_scene::{Scene, SceneId};

struct Maps {
    material: Option<String>,
    texture: Option<MapRef>,
    maps: DecalMaps,
    used: u64,
}

#[derive(Default)]
pub(crate) struct KeptDecals {
    maps: HashMap<u32, Maps>,
    library: Option<(SceneId, Revision)>,
    frame: u64,
    pub(crate) uploaded: Vec<GpuDecal>,
}

impl KeptDecals {
    pub(crate) fn begin(&mut self, scene: &Scene) {
        let library = Some((scene.id(), scene.materials.revision()));
        if self.library != library {
            self.maps.clear();
            self.library = library;
        }
        self.frame += 1;
    }

    pub(crate) fn read(&mut self, scene: &Scene, decal: &Decal) {
        let frame = self.frame;
        let entry = self.maps.entry(decal.id);
        let kept = entry.or_insert_with(|| Maps {
            material: None,
            texture: None,
            maps: Default::default(),
            used: 0,
        });
        if kept.used == 0 || kept.material != decal.material || kept.texture != decal.texture {
            let material = super::gpu::resolve(&scene.materials, decal);
            kept.maps = record::maps(decal, material);
            kept.material.clone_from(&decal.material);
            kept.texture.clone_from(&decal.texture);
        }
        kept.used = frame;
    }

    pub(crate) fn of(&self, id: u32) -> &DecalMaps {
        &self.maps[&id].maps
    }

    pub(crate) fn end(&mut self) {
        let frame = self.frame;
        self.maps.retain(|_, m| m.used == frame);
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;
    use rusty_components::MaterialAsset;
    use rusty_scene::decal::DecalSpec;

    fn albedo(kept: &mut KeptDecals, scene: &Scene) -> Option<MapRef> {
        kept.begin(scene);
        let decal = &scene.decals.iter().next().expect("a decal");
        kept.read(scene, decal);
        let map = kept.of(decal.id)[0].clone();
        kept.end();
        map
    }

    #[test]
    fn a_decals_maps_follow_its_material_and_the_library() {
        let mut scene = Scene::new();
        let paint = |map: &str| MaterialAsset {
            base_color_map: Some(map.into()),
            ..MaterialAsset::default()
        };
        scene.materials.insert("paint".into(), paint("a.png"));
        let spec = DecalSpec {
            material: Some("paint".into()),
            ..DecalSpec::default()
        };
        let id = scene.spawn_decal(Vec3::ZERO, Vec3::Y, spec).unwrap();
        let mut kept = KeptDecals::default();
        assert_eq!(albedo(&mut kept, &scene), Some("a.png".into()));
        scene.materials.insert("paint".into(), paint("b.png"));
        assert_eq!(
            albedo(&mut kept, &scene),
            Some("b.png".into()),
            "the library"
        );
        scene.decals.set_material(id, None);
        assert_eq!(albedo(&mut kept, &scene), None, "its own material");
        scene.clear_decals(false);
        kept.begin(&scene);
        kept.end();
        assert!(kept.maps.is_empty(), "a decal gone drops its maps");
    }
}

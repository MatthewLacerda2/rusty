//! The solid path's group order, read from the layouts' own builders, and the
//! engine's WGSL `@group` of each binding held to the Rust layout at that index,
//! so a reorder on one side alone fails here.

use super::*;
use crate::gpu::bind_layouts as layouts;
use crate::passes::blended::Globals;
use crate::passes::particles::gpu::{draw_layout_entries, step_layout_entries};
use crate::passes::shadows::{caster_entries, light_entries};

/// What a WGSL binding is, as far as a layout entry can disagree with it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Uniform,
    Storage,
    Texture,
    Sampler,
}

/// One `@group(g) @binding(b) var… name: type` of a module.
#[derive(Debug)]
struct Decl {
    group: u32,
    binding: u32,
    kind: Kind,
    name: String,
}

fn kind_of(ty: &wgpu::BindingType) -> Kind {
    match ty {
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            ..
        } => Kind::Uniform,
        wgpu::BindingType::Buffer { .. } => Kind::Storage,
        wgpu::BindingType::Sampler(_) => Kind::Sampler,
        _ => Kind::Texture,
    }
}

/// The number in `text`'s leading `(<n>)`.
fn number(text: &str) -> u32 {
    let end = text.find(')').expect("a closing paren");
    text[..end].trim().parse().expect("a binding number")
}

/// Every binding `wgsl` declares.
fn declared(wgsl: &str) -> Vec<Decl> {
    let mut out = Vec::new();
    for piece in wgsl.split("@group(").skip(1) {
        let group = number(piece);
        let at = piece.find("@binding(").expect("a group's binding");
        let rest = &piece[at + "@binding(".len()..];
        let binding = number(rest);
        let var = &rest[rest.find("var").expect("a binding's var") + 3..];
        let (space, var) = match var.strip_prefix('<') {
            Some(v) => v.split_once('>').expect("a closed address space"),
            None => ("", var),
        };
        let (name, ty) = var.split_once(':').expect("a typed binding");
        let ty = ty.trim_start();
        let kind = match space {
            "uniform" => Kind::Uniform,
            s if s.starts_with("storage") => Kind::Storage,
            _ if ty.starts_with("sampler") => Kind::Sampler,
            _ => Kind::Texture,
        };
        let name = name.trim().to_string();
        out.push(Decl {
            group,
            binding,
            kind,
            name,
        });
    }
    out
}

fn engine_wgsl(path: &str) -> String {
    let dir = rusty_shadergen::engine_shader_dir();
    std::fs::read_to_string(format!("{dir}/{path}")).expect("an engine shader")
}

/// `decl` has an entry of its kind at its binding in `groups`' layout at its index.
fn assert_in_layout(decl: &Decl, groups: &[Option<&[wgpu::BindGroupLayoutEntry]>; COUNT]) {
    let layout = groups
        .get(decl.group as usize)
        .copied()
        .flatten()
        .unwrap_or_else(|| {
            panic!(
                "{} sits at group {}, a gap in its layout",
                decl.name, decl.group
            )
        });
    let entry = layout.iter().find(|e| e.binding == decl.binding);
    let found = entry.map(|e| kind_of(&e.ty));
    assert_eq!(
        found,
        Some(decl.kind),
        "`{}` (@group({}) @binding({})) is not what the Rust layout at that index holds: \
         read the index from `rusty_shadergen::groups` on both sides",
        decl.name,
        decl.group,
        decl.binding
    );
}

/// A layout of all four groups, by role.
fn four<T>(global: T, scene: T, material: T, draw: T) -> Groups<T> {
    Groups {
        global,
        scene: Some(scene),
        material: Some(material),
        draw,
    }
}

/// A shadow caster's layout: the light, the casters per draw, the material when
/// it clips, no scene group.
fn caster<T>(global: T, draw: T, material: Option<T>) -> Groups<T> {
    Groups {
        global,
        scene: None,
        material,
        draw,
    }
}

/// What a surface shader declares beside the engine's bindings: its param uniform
/// and each extra map slot, all in the material group.
fn authored() -> String {
    let mut wgsl = rusty_shadergen::params::uniform_decl();
    for slot in rusty_shadergen::textures::SLOTS {
        wgsl.push_str(&rusty_shadergen::textures::decl(slot));
    }
    wgsl
}

#[test]
fn the_indices_put_the_per_draw_group_last_and_the_material_below_it() {
    assert_eq!(DRAW as usize, COUNT - 1);
    assert_eq!(MATERIAL + 1, DRAW);
    let mut all = [GLOBAL, SCENE, MATERIAL, DRAW];
    all.sort_unstable();
    assert_eq!(all, [0, 1, 2, 3], "one index each");
}

#[test]
fn every_solid_layout_puts_its_draw_group_last_and_its_material_below_it() {
    let solid = [
        ("forward", four("global", "scene", "material", "draw")),
        ("particles", four("global", "depth", "material", "draw")),
        ("shadow clip", caster("light", "draw", Some("material"))),
    ];
    for (name, groups) in solid {
        let at = groups.ordered();
        assert_eq!(at[COUNT - 1], Some("draw"), "{name}: {at:?}");
        assert_eq!(at[COUNT - 2], Some("material"), "{name}: {at:?}");
    }
    let plain = caster("light", "draw", None).ordered();
    assert_eq!(plain, [Some("light"), None, None, Some("draw")]);
}

#[test]
fn the_forward_wgsl_declares_each_binding_in_its_rust_group() {
    let (g, d, m, s) = (
        layouts::camera_lighting_entries(),
        layouts::entity_bones_entries(),
        layouts::material_entries(),
        layouts::shadow_entries(),
    );
    let groups = four(&g[..], &s[..], &m[..], &d[..]).ordered();
    let decls = declared(&engine_wgsl("forward/bindings.wgsl"));
    assert!(decls.len() > 40, "parsed {} bindings", decls.len());
    for decl in &decls {
        assert_in_layout(decl, &groups);
    }
    // What a surface shader adds: its param uniform and each extra map slot.
    for decl in &declared(&authored()) {
        assert_eq!(decl.group, MATERIAL, "{decl:?}");
        assert_in_layout(decl, &groups);
    }
}

#[test]
fn the_shadow_stage_declares_each_caster_binding_at_its_rust_group() {
    let (light, casters, material) = (
        light_entries(),
        caster_entries(),
        layouts::material_entries(),
    );
    let groups = caster(&light[..], &casters[..], Some(&material[..])).ordered();
    let mut wgsl = engine_wgsl("forward/bindings.wgsl");
    wgsl.push_str(&engine_wgsl("forward/shadow_caster.wgsl"));
    wgsl.push_str(&authored());
    let decls = declared(&wgsl);
    for (index, entries) in groups.iter().enumerate() {
        for entry in entries.iter().flat_map(|e| e.iter()) {
            let found = decls
                .iter()
                .find(|d| d.group as usize == index && d.binding == entry.binding);
            assert_eq!(
                found.map(|d| d.kind),
                Some(kind_of(&entry.ty)),
                "the shadow layout's group {index} binding {} has no WGSL twin there",
                entry.binding
            );
        }
    }
}

#[test]
fn the_particle_wgsl_declares_each_binding_in_its_rust_group() {
    let (global, depth, sprite) = (
        layouts::camera_lighting_entries(),
        layouts::scene_depth_entries(),
        layouts::texture_entries(),
    );
    let (globals, draw, step) = (
        Globals::entries(),
        draw_layout_entries(),
        step_layout_entries(),
    );
    // The CPU path binds the pass globals per draw, a GPU emitter its own group.
    let cpu = four(&global[..], &depth[..], &sprite[..], &globals[..]).ordered();
    let gpu = four(&global[..], &depth[..], &sprite[..], &draw[..]).ordered();
    let emitter = declared(&engine_wgsl("particles/gpu.wgsl"));
    let drawn = declared(&engine_wgsl("particles.wgsl"));
    assert!(drawn.len() > 10, "parsed {} bindings", drawn.len());
    for decl in drawn.iter().chain(&emitter) {
        let groups = if decl.name.starts_with("gpu_") {
            &gpu
        } else {
            &cpu
        };
        assert_in_layout(decl, groups);
        if decl.group == DRAW && !decl.name.starts_with("gpu_") {
            assert_in_layout(decl, &gpu);
        }
    }
    // The step shares the emitter's declarations, in its own group at `DRAW`.
    let stepped = draw_only(&step[..]);
    for decl in declared(&engine_wgsl("particles_step.wgsl"))
        .iter()
        .chain(&emitter)
    {
        assert_in_layout(decl, &stepped);
    }
}

#[test]
fn a_swapped_group_is_caught() {
    let (g, d, m, s) = (
        layouts::camera_lighting_entries(),
        layouts::entity_bones_entries(),
        layouts::material_entries(),
        layouts::shadow_entries(),
    );
    // The draw and scene groups swapped on the Rust side alone.
    let groups = four(&g[..], &d[..], &m[..], &s[..]).ordered();
    let decls = declared(&engine_wgsl("forward/bindings.wgsl"));
    let caught = std::panic::catch_unwind(|| {
        for decl in &decls {
            assert_in_layout(decl, &groups);
        }
    });
    assert!(caught.is_err(), "a swap must fail the agreement check");
}

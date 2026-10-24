//! src/shadergen/textures.rs — the extra texture slots a surface block may sample
//! (#400): a material-named pattern (a baked noise for dissolve, a detail map).
//!
//! The slot list is curated, like the block catalog: v1 has **one**, `mask`, and a
//! slot is added only when a real block needs a second. Each slot has a fixed
//! group-2 binding after the param uniform, so every material bind group carries it
//! (the renderer binds a 1×1 white fallback when a material names none, or its file
//! is missing). A surface variant *declares* a slot only when one of its blocks reads
//! it ([`Block::textures`](super::blocks::Block)), so a variant without one keeps the
//! plain contract.
//!
//! A material names the texture per slot (`MaterialAsset::shader_textures`), and a
//! slot it names that is not in [`SLOTS`] is refused by name (#395).

/// One extra texture slot: its name (the material key and the block's reference)
/// and its group-2 binding.
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub name: &'static str,
    pub binding: u32,
}

/// The `mask` slot's group-2 binding (after the param uniform at 6).
const MASK_BINDING: u32 = 7;

/// Every extra texture slot, in binding order.
pub const SLOTS: &[Slot] = &[Slot {
    name: "mask",
    binding: MASK_BINDING,
}];

/// Look up a slot by name, refusing an unknown one with the list of known slots.
pub fn slot(name: &str) -> Result<&'static Slot, String> {
    SLOTS.iter().find(|s| s.name == name).ok_or_else(|| {
        let known: Vec<&str> = SLOTS.iter().map(|s| s.name).collect();
        format!(
            "unknown shader texture slot {name:?}; slots: {}",
            known.join(", ")
        )
    })
}

/// How far past a material texture's binding its own sampler sits (#1323): each
/// map samples with the wrap and filter its import settings chose. Albedo is the
/// one exception, its sampler `s_diffuse` staying at binding 1.
pub const SAMPLER_OFFSET: u32 = 16;

/// The group-2 binding of the sampler paired with the texture at `texture_binding`.
pub const fn sampler_binding(texture_binding: u32) -> u32 {
    texture_binding + SAMPLER_OFFSET
}

/// The WGSL declaring `slot` in a surface variant: `t_<name>` at its binding and
/// its sampler `s_<name>` at [`sampler_binding`].
pub fn decl(slot: &Slot) -> String {
    format!(
        "@group(2) @binding({}) var t_{name}: texture_2d<f32>;\n\
         @group(2) @binding({}) var s_{name}: sampler;\n",
        slot.binding,
        sampler_binding(slot.binding),
        name = slot.name
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_is_a_slot_and_an_unknown_name_is_refused_by_name() {
        assert_eq!(slot("mask").unwrap().binding, MASK_BINDING);
        let err = slot("maks").unwrap_err();
        assert!(err.contains("\"maks\"") && err.contains("mask"), "{err}");
    }

    #[test]
    fn slots_sit_after_the_param_uniform_in_binding_order() {
        let first = super::super::params::PARAM_BINDING + 1;
        for (expected, s) in (first..).zip(SLOTS) {
            assert_eq!(s.binding, expected, "{}", s.name);
        }
    }

    #[test]
    fn decl_names_the_texture_after_its_slot() {
        assert_eq!(
            decl(slot("mask").unwrap()),
            "@group(2) @binding(7) var t_mask: texture_2d<f32>;\n\
             @group(2) @binding(23) var s_mask: sampler;\n"
        );
    }
}

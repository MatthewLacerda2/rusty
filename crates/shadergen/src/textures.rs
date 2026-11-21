#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub name: &'static str,
    pub binding: u32,
}

const MASK_BINDING: u32 = 7;

pub const SLOTS: &[Slot] = &[Slot {
    name: "mask",
    binding: MASK_BINDING,
}];

pub fn slot(name: &str) -> Result<&'static Slot, String> {
    SLOTS.iter().find(|s| s.name == name).ok_or_else(|| {
        let known: Vec<&str> = SLOTS.iter().map(|s| s.name).collect();
        format!(
            "unknown shader texture slot {name:?}; slots: {}",
            known.join(", ")
        )
    })
}

pub const SAMPLER_OFFSET: u32 = 16;

pub const fn sampler_binding(texture_binding: u32) -> u32 {
    texture_binding + SAMPLER_OFFSET
}

pub fn decl(slot: &Slot) -> String {
    format!(
        "@group({group}) @binding({}) var t_{name}: texture_2d<f32>;\n\
         @group({group}) @binding({}) var s_{name}: sampler;\n",
        slot.binding,
        sampler_binding(slot.binding),
        name = slot.name,
        group = super::groups::MATERIAL,
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
            "@group(1) @binding(7) var t_mask: texture_2d<f32>;\n\
             @group(1) @binding(23) var s_mask: sampler;\n"
        );
    }
}

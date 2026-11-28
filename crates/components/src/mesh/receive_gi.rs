rusty_core::named_enum! {
    /// Where a static mesh takes its baked indirect light from — Unity's *Receive Global
    /// Illumination*. `Lightmaps` is the default, and what a file without the field reads.
    /// A `LightProbes` mesh still blocks and bounces light in the bake as its `cast_shadows`
    /// says, but gets no lightmap texels and reads every loaded scene's light probes at runtime.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ReceiveGi {
        #[default]
        Lightmaps = "Lightmaps",
        LightProbes = "LightProbes",
    }
}

#[cfg(test)]
mod tests {
    use super::ReceiveGi;

    #[test]
    fn modes_are_stored_by_unity_name_and_lightmaps_by_default() {
        assert_eq!(ReceiveGi::default(), ReceiveGi::Lightmaps);
        let json = serde_json::to_string(&ReceiveGi::LightProbes).unwrap();
        assert_eq!(json, "\"LightProbes\"");
        let read: ReceiveGi = serde_json::from_str("\"lightprobes\"").unwrap();
        assert_eq!(read, ReceiveGi::LightProbes);
    }
}

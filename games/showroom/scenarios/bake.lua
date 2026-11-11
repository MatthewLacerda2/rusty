-- scenarios/bake.lua — with `bake = true` the recipe's tiny lightmap bake
-- finished, was written beside the scene, and is applied after the scene is
-- loaded from disk. Without the parameter it is skipped: the recipe leaves a
-- `BakeRequested` entity only when it baked, and CI runs the defaults.

if Scene.FindEntityByName("BakeRequested") == nil then
    Harness.Log("skipped: the showroom was generated without bake = true")
    return
end

local report = Lighting.GetBakeReport("lightmaps")
Harness.Expect(report ~= nil, "the lightmap bake wrote its report beside the scene")
Harness.Expect(report ~= nil and report.maps > 0, "the bake made at least one lightmap")
Harness.Expect(Lighting.GetBakeReport("probes") ~= nil, "the probe bake ran with it")
Harness.Expect(Lighting.GetBakeReport("reflections") ~= nil, "and the reflection bake")

if Harness.Render() then
    Harness.Expect(Debug.Stats().lightmap_bytes.last > 0,
        "the loaded scene draws with its lightmaps bound")
else
    Harness.Log("lightmap binding unchecked: no GPU adapter")
end

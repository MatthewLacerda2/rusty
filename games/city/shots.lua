-- The fixed cameras the city is screenshotted from, at the recipe's defaults
-- (`size = 400`: two blocks across, five deep).
-- A shot moves the main camera, the Street viewpoint, but every active camera
-- draws, so the other viewpoints are turned off first, as Play's
-- `scripts/viewpoints.lua` does.

for n, camera in ipairs(Scene.GetChildren(Scene.FindEntityByName("Viewpoints"))) do
    if n > 1 then Scene.SetActive(camera, false) end
end

return {
    { name = "street", position = { 203, 1.7, 20 }, target = { 203, 1.7, 390 } },
    { name = "above", position = { 40, 160, -60 }, target = { 200, 0, 200 } },
}

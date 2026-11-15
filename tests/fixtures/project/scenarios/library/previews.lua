-- tests/fixtures/project/scenarios/library/previews.lua — the look behind
-- `make library-previews OUT=<dir>`: every material of the engine's graph library
-- (engine/textures/library/, found through Texture.Ops()) baked at 1024² and
-- rendered with Debug.PreviewMaterial at 1024², on the sphere and on the plane with
-- `repeats = 3`, at wear 0.3 and 1. Writes <OUT>/<material>_<mesh>_wear<w>.png.
--
-- The make verb runs it on a throwaway copy of the fixture project (the bakes land
-- in that project's assets/textures/) and sets OUT and ONLY above this file. ONLY,
-- when set, names the one material to render.

local LIBRARY = "engine/textures/library/"
-- Metres one repeat covers: the material's world_size (docs/api/Texture.md, *The library*).
local WORLD = {
  concrete_poured = 2, brick_wall = 1, plaster_painted = 2, metal_painted_worn = 1,
  steel_tread_plate = 0.5, wood_planks = 2, ceramic_tile = 1, asphalt = 2,
  ground_sand_pebbles = 2, fabric_carpet = 0.5,
}

local function materials()
  local found = {}
  for _, o in ipairs(Texture.Ops()) do
    local name = o.path and o.path:match("^" .. LIBRARY .. "([%w_]+)%.graph%.json$")
    if name and (ONLY == nil or ONLY == name) then
      found[#found + 1] = { name = name, path = o.path, metallic = false }
    end
  end
  return found
end

for _, m in ipairs(materials()) do
  for _, wear in ipairs({ 0.3, 1 }) do
    local id = m.name .. "_wear" .. wear
    local outputs = { base_color = "m", normal = "m.normal", roughness = "m.roughness",
      occlusion = "m.occlusion" }
    local recipe = { resolution = 1024, outputs = outputs, nodes = {
      { id = "m", op = "graph", path = m.path, params = { wear = wear } } } }
    local ok = pcall(Texture.Stats, { resolution = 16, nodes = {
      { id = "m", op = "graph", path = m.path } }, output = "m.metallic" })
    if ok then outputs.metallic = "m.metallic" end
    Material.DefineAsset(id, { uv_mapping = "Triplanar", world_size = WORLD[m.name] or 1,
      maps = recipe })
    Debug.PreviewMaterial(id, OUT .. "/" .. id .. "_sphere.png", { resolution = 1024 })
    Debug.PreviewMaterial(id, OUT .. "/" .. id .. "_plane.png",
      { mesh = "plane", repeats = 3, resolution = 1024 })
    Harness.Log("previewed " .. id)
  end
end

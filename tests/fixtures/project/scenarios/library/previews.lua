-- Behind `make library-previews OUT=<dir>`: every library material baked and rendered with
-- Debug.PreviewMaterial at 1024², on the sphere and the plane (`repeats = 3`), at wear 0.3 and 1,
-- written to <OUT>/<material>_wear<w>_<mesh>.png. ONLY, when set, names the one material; it
-- raises when nothing would be rendered. Bakes land in the project's assets/textures/, so the
-- make verb runs this on a throwaway copy of the fixture project.

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
      found[#found + 1] = { name = name, path = o.path }
    end
  end
  if #found == 0 then
    error("library-previews: no library material is named " .. tostring(ONLY))
  end
  return found
end

-- Whether the graph at `path` has a `metallic` output. Only that answer is caught:
-- any other failure of the probe is a broken graph, and raises.
local function has_metallic(path)
  local ok, err = pcall(Texture.Stats, { resolution = 16, nodes = {
    { id = "m", op = "graph", path = path } }, output = "m.metallic" }, 16)
  if ok then return true end
  if tostring(err):find('has no output "metallic"', 1, true) then return false end
  error(err, 0)
end

local function preview(id, file, opts)
  if Debug.PreviewMaterial(id, file, opts) == nil then
    error("library-previews: no GPU on this machine, nothing rendered")
  end
end

for _, m in ipairs(materials()) do
  local metallic = has_metallic(m.path)
  for _, wear in ipairs({ 0.3, 1 }) do
    local id = m.name .. "_wear" .. wear
    local outputs = { base_color = "m", normal = "m.normal", roughness = "m.roughness",
      occlusion = "m.occlusion" }
    if metallic then outputs.metallic = "m.metallic" end
    local recipe = { resolution = 1024, outputs = outputs, nodes = {
      { id = "m", op = "graph", path = m.path, params = { wear = wear } } } }
    Material.DefineAsset(id, { uv_mapping = "Triplanar", world_size = WORLD[m.name] or 1,
      maps = recipe })
    preview(id, OUT .. "/" .. id .. "_sphere.png", { resolution = 1024 })
    preview(id, OUT .. "/" .. id .. "_plane.png",
      { mesh = "plane", repeats = 3, resolution = 1024 })
    Harness.Log("previewed " .. id)
  end
end
